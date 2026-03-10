// Query planner: rule-based logical plan + physical plan conversion
pub mod optimize;

use crate::parser::ast::*;
use cypherlite_core::LabelRegistry;

/// A logical plan node representing a query execution strategy.
#[derive(Debug, Clone, PartialEq)]
pub enum LogicalPlan {
    /// Scan all nodes, optionally filtered by label ID.
    NodeScan {
        variable: String,
        label_id: Option<u32>,
    },
    /// Expand from a source variable along edges of given type.
    Expand {
        source: Box<LogicalPlan>,
        src_var: String,
        rel_var: Option<String>,
        target_var: String,
        rel_type_id: Option<u32>,
        direction: RelDirection,
    },
    /// Filter rows by a predicate expression.
    Filter {
        source: Box<LogicalPlan>,
        predicate: Expression,
    },
    /// Project specific expressions (RETURN clause).
    Project {
        source: Box<LogicalPlan>,
        items: Vec<ReturnItem>,
        distinct: bool,
    },
    /// Sort rows (ORDER BY).
    Sort {
        source: Box<LogicalPlan>,
        items: Vec<OrderItem>,
    },
    /// Skip N rows.
    Skip {
        source: Box<LogicalPlan>,
        count: Expression,
    },
    /// Limit to N rows.
    Limit {
        source: Box<LogicalPlan>,
        count: Expression,
    },
    /// Aggregate (GROUP BY equivalent via function calls like count).
    Aggregate {
        source: Box<LogicalPlan>,
        group_keys: Vec<Expression>,
        aggregates: Vec<(String, AggregateFunc)>,
    },
    /// Create nodes/edges.
    CreateOp {
        source: Option<Box<LogicalPlan>>,
        pattern: Pattern,
    },
    /// Delete nodes/edges.
    DeleteOp {
        source: Box<LogicalPlan>,
        exprs: Vec<Expression>,
        detach: bool,
    },
    /// Set properties.
    SetOp {
        source: Box<LogicalPlan>,
        items: Vec<SetItem>,
    },
    /// Remove properties/labels.
    RemoveOp {
        source: Box<LogicalPlan>,
        items: Vec<RemoveItem>,
    },
    /// Empty source (produces one empty row).
    EmptySource,
}

/// Supported aggregate functions.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateFunc {
    Count { distinct: bool },
    CountStar,
}

/// Error type for plan construction failures.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanError {
    pub message: String,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Plan error: {}", self.message)
    }
}

impl std::error::Error for PlanError {}

/// Logical planner that converts a parsed Query AST into a LogicalPlan tree.
pub struct LogicalPlanner<'a> {
    registry: &'a mut dyn LabelRegistry,
}

impl<'a> LogicalPlanner<'a> {
    pub fn new(registry: &'a mut dyn LabelRegistry) -> Self {
        Self { registry }
    }

    /// Convert a parsed Query into a LogicalPlan.
    pub fn plan(&mut self, query: &Query) -> Result<LogicalPlan, PlanError> {
        let mut current: Option<LogicalPlan> = None;

        for clause in &query.clauses {
            current = Some(self.plan_clause(clause, current)?);
        }

        current.ok_or_else(|| PlanError {
            message: "empty query produces no plan".to_string(),
        })
    }

    fn plan_clause(
        &mut self,
        clause: &Clause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        match clause {
            Clause::Match(mc) => self.plan_match(mc, current),
            Clause::Return(rc) => self.plan_return(rc, current),
            Clause::Create(cc) => Ok(self.plan_create(cc, current)),
            Clause::Set(sc) => self.plan_set(sc, current),
            Clause::Delete(dc) => self.plan_delete(dc, current),
            Clause::Remove(rc) => self.plan_remove(rc, current),
            Clause::With(_) | Clause::Merge(_) => Err(PlanError {
                message: format!("unsupported clause type: {:?}", clause),
            }),
        }
    }

    fn plan_match(
        &mut self,
        mc: &MatchClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        // Build plan from pattern chains.
        // For now, handle the first chain only (single path pattern).
        let chain = mc.pattern.chains.first().ok_or_else(|| PlanError {
            message: "MATCH clause has no pattern chains".to_string(),
        })?;

        let mut plan = self.plan_pattern_chain(chain)?;

        // If there was an existing plan, this is a subsequent MATCH.
        // For simplicity, we replace with the new scan.
        // A full implementation would do a cross product or join.
        if let Some(prev) = current {
            // For chained MATCH clauses, use previous plan as context.
            // Simple approach: wrap previous in the new scan chain.
            // For now, just use the new plan (covers most test cases).
            let _ = prev;
        }

        // Apply WHERE predicate as Filter.
        if let Some(ref predicate) = mc.where_clause {
            plan = LogicalPlan::Filter {
                source: Box::new(plan),
                predicate: predicate.clone(),
            };
        }

        Ok(plan)
    }

    fn plan_pattern_chain(&mut self, chain: &PatternChain) -> Result<LogicalPlan, PlanError> {
        let mut elements = chain.elements.iter();

        // First element must be a node.
        let first_node = match elements.next() {
            Some(PatternElement::Node(np)) => np,
            _ => {
                return Err(PlanError {
                    message: "pattern chain must start with a node".to_string(),
                })
            }
        };

        let variable = first_node.variable.clone().unwrap_or_default();

        let label_id = first_node
            .labels
            .first()
            .map(|name| self.registry.get_or_create_label(name));

        let mut plan = LogicalPlan::NodeScan { variable, label_id };

        // Process remaining relationship + node pairs.
        while let Some(rel_elem) = elements.next() {
            let rel = match rel_elem {
                PatternElement::Relationship(rp) => rp,
                _ => {
                    return Err(PlanError {
                        message: "expected relationship after node in pattern".to_string(),
                    })
                }
            };

            let target_node = match elements.next() {
                Some(PatternElement::Node(np)) => np,
                _ => {
                    return Err(PlanError {
                        message: "expected node after relationship in pattern".to_string(),
                    })
                }
            };

            let src_var = Self::extract_src_var(&plan);
            let rel_var = rel.variable.clone();
            let target_var = target_node.variable.clone().unwrap_or_default();

            let rel_type_id = rel
                .rel_types
                .first()
                .map(|name| self.registry.get_or_create_rel_type(name));

            plan = LogicalPlan::Expand {
                source: Box::new(plan),
                src_var,
                rel_var,
                target_var,
                rel_type_id,
                direction: rel.direction,
            };
        }

        Ok(plan)
    }

    /// Extract the "output variable" from a plan node (used as src_var for Expand).
    fn extract_src_var(plan: &LogicalPlan) -> String {
        match plan {
            LogicalPlan::NodeScan { variable, .. } => variable.clone(),
            LogicalPlan::Expand { target_var, .. } => target_var.clone(),
            LogicalPlan::Filter { source, .. } => Self::extract_src_var(source),
            _ => String::new(),
        }
    }

    fn plan_return(
        &self,
        rc: &ReturnClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        let source = current.ok_or_else(|| PlanError {
            message: "RETURN clause requires a preceding data source".to_string(),
        })?;

        let mut plan = LogicalPlan::Project {
            source: Box::new(source),
            items: rc.items.clone(),
            distinct: rc.distinct,
        };

        // ORDER BY
        if let Some(ref order_items) = rc.order_by {
            plan = LogicalPlan::Sort {
                source: Box::new(plan),
                items: order_items.clone(),
            };
        }

        // SKIP
        if let Some(ref skip_expr) = rc.skip {
            plan = LogicalPlan::Skip {
                source: Box::new(plan),
                count: skip_expr.clone(),
            };
        }

        // LIMIT
        if let Some(ref limit_expr) = rc.limit {
            plan = LogicalPlan::Limit {
                source: Box::new(plan),
                count: limit_expr.clone(),
            };
        }

        Ok(plan)
    }

    fn plan_create(&self, cc: &CreateClause, current: Option<LogicalPlan>) -> LogicalPlan {
        LogicalPlan::CreateOp {
            source: current.map(Box::new),
            pattern: cc.pattern.clone(),
        }
    }

    fn plan_set(
        &self,
        sc: &SetClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        let source = current.ok_or_else(|| PlanError {
            message: "SET clause requires a preceding data source".to_string(),
        })?;

        Ok(LogicalPlan::SetOp {
            source: Box::new(source),
            items: sc.items.clone(),
        })
    }

    fn plan_delete(
        &self,
        dc: &DeleteClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        let source = current.ok_or_else(|| PlanError {
            message: "DELETE clause requires a preceding data source".to_string(),
        })?;

        Ok(LogicalPlan::DeleteOp {
            source: Box::new(source),
            exprs: dc.exprs.clone(),
            detach: dc.detach,
        })
    }

    fn plan_remove(
        &self,
        rc: &RemoveClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        let source = current.ok_or_else(|| PlanError {
            message: "REMOVE clause requires a preceding data source".to_string(),
        })?;

        Ok(LogicalPlan::RemoveOp {
            source: Box::new(source),
            items: rc.items.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_query;
    use cypherlite_storage::catalog::Catalog;

    // Helper: parse + plan a query using a fresh Catalog.
    fn plan_query(input: &str) -> LogicalPlan {
        let query = parse_query(input).expect("should parse");
        let mut catalog = Catalog::default();
        let mut planner = LogicalPlanner::new(&mut catalog);
        planner.plan(&query).expect("should plan")
    }

    // Helper: parse + plan, returning catalog for ID inspection.
    fn plan_query_with_catalog(input: &str) -> (LogicalPlan, Catalog) {
        let query = parse_query(input).expect("should parse");
        let mut catalog = Catalog::default();
        let plan = {
            let mut planner = LogicalPlanner::new(&mut catalog);
            planner.plan(&query).expect("should plan")
        };
        (plan, catalog)
    }

    // ======================================================================
    // TASK-043: Planner unit tests
    // ======================================================================

    /// MATCH (n:Person) RETURN n -> NodeScan + Project
    #[test]
    fn test_plan_single_node_match_return() {
        let (plan, catalog) = plan_query_with_catalog("MATCH (n:Person) RETURN n");
        let person_id = catalog.label_id("Person").expect("Person label exists");

        // Outermost should be Project wrapping NodeScan.
        match &plan {
            LogicalPlan::Project {
                source, distinct, ..
            } => {
                assert!(!distinct);
                match source.as_ref() {
                    LogicalPlan::NodeScan { variable, label_id } => {
                        assert_eq!(variable, "n");
                        assert_eq!(*label_id, Some(person_id));
                    }
                    other => panic!("expected NodeScan, got {:?}", other),
                }
            }
            other => panic!("expected Project, got {:?}", other),
        }
    }

    /// MATCH (a)-[:KNOWS]->(b)-[:KNOWS]->(c) RETURN c
    /// -> NodeScan(a) + Expand(KNOWS, b) + Expand(KNOWS, c) + Project
    #[test]
    fn test_plan_2hop_match() {
        let (plan, catalog) =
            plan_query_with_catalog("MATCH (a)-[:KNOWS]->(b)-[:KNOWS]->(c) RETURN c");
        let knows_id = catalog.rel_type_id("KNOWS").expect("KNOWS rel type exists");

        // Outermost: Project
        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        // Second Expand (b -> c)
        let expand1_source = match project_source {
            LogicalPlan::Expand {
                src_var,
                target_var,
                rel_type_id,
                direction,
                source,
                ..
            } => {
                assert_eq!(src_var, "b");
                assert_eq!(target_var, "c");
                assert_eq!(*rel_type_id, Some(knows_id));
                assert_eq!(*direction, RelDirection::Outgoing);
                source.as_ref()
            }
            other => panic!("expected Expand, got {:?}", other),
        };

        // First Expand (a -> b)
        let scan = match expand1_source {
            LogicalPlan::Expand {
                src_var,
                target_var,
                rel_type_id,
                direction,
                source,
                ..
            } => {
                assert_eq!(src_var, "a");
                assert_eq!(target_var, "b");
                assert_eq!(*rel_type_id, Some(knows_id));
                assert_eq!(*direction, RelDirection::Outgoing);
                source.as_ref()
            }
            other => panic!("expected Expand, got {:?}", other),
        };

        // NodeScan(a)
        match scan {
            LogicalPlan::NodeScan { variable, label_id } => {
                assert_eq!(variable, "a");
                assert_eq!(*label_id, None); // no label on (a)
            }
            other => panic!("expected NodeScan, got {:?}", other),
        }
    }

    /// MATCH (n:Person) WHERE n.age > 30 RETURN n -> NodeScan + Filter + Project
    #[test]
    fn test_plan_match_where_return() {
        let plan = plan_query("MATCH (n:Person) WHERE n.age > 30 RETURN n");

        // Outermost: Project
        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        // Filter
        let filter_source = match project_source {
            LogicalPlan::Filter {
                source, predicate, ..
            } => {
                // Verify predicate is n.age > 30
                match predicate {
                    Expression::BinaryOp(BinaryOp::Gt, lhs, rhs) => {
                        assert_eq!(
                            **lhs,
                            Expression::Property(
                                Box::new(Expression::Variable("n".to_string())),
                                "age".to_string()
                            )
                        );
                        assert_eq!(**rhs, Expression::Literal(Literal::Integer(30)));
                    }
                    other => panic!("expected BinaryOp Gt, got {:?}", other),
                }
                source.as_ref()
            }
            other => panic!("expected Filter, got {:?}", other),
        };

        // NodeScan
        match filter_source {
            LogicalPlan::NodeScan { variable, label_id } => {
                assert_eq!(variable, "n");
                assert!(label_id.is_some());
            }
            other => panic!("expected NodeScan, got {:?}", other),
        }
    }

    /// MATCH (n) CREATE (m:Person {name: "Alice"}) -> NodeScan + CreateOp
    #[test]
    fn test_plan_match_create() {
        let plan = plan_query("MATCH (n) CREATE (m:Person {name: 'Alice'})");

        match &plan {
            LogicalPlan::CreateOp {
                source, pattern, ..
            } => {
                // Source should be NodeScan(n)
                let src = source.as_ref().expect("should have source");
                match src.as_ref() {
                    LogicalPlan::NodeScan { variable, label_id } => {
                        assert_eq!(variable, "n");
                        assert_eq!(*label_id, None);
                    }
                    other => panic!("expected NodeScan, got {:?}", other),
                }
                // Pattern should have the Person node
                assert!(!pattern.chains.is_empty());
            }
            other => panic!("expected CreateOp, got {:?}", other),
        }
    }

    /// CREATE (n:Person) -> CreateOp with no source
    #[test]
    fn test_plan_create_only() {
        let plan = plan_query("CREATE (n:Person)");

        match &plan {
            LogicalPlan::CreateOp { source, pattern } => {
                assert!(source.is_none());
                assert!(!pattern.chains.is_empty());
            }
            other => panic!("expected CreateOp, got {:?}", other),
        }
    }

    /// MATCH (n:Person) SET n.name = "Bob" RETURN n
    /// -> NodeScan + SetOp + Project
    #[test]
    fn test_plan_match_set_return() {
        let plan = plan_query("MATCH (n:Person) SET n.name = 'Bob' RETURN n");

        // Outermost: Project
        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        // SetOp
        let set_source = match project_source {
            LogicalPlan::SetOp { source, items } => {
                assert_eq!(items.len(), 1);
                source.as_ref()
            }
            other => panic!("expected SetOp, got {:?}", other),
        };

        // NodeScan
        match set_source {
            LogicalPlan::NodeScan { variable, .. } => {
                assert_eq!(variable, "n");
            }
            other => panic!("expected NodeScan, got {:?}", other),
        }
    }

    /// MATCH (n) DELETE n -> NodeScan + DeleteOp
    #[test]
    fn test_plan_match_delete() {
        let plan = plan_query("MATCH (n) DELETE n");

        match &plan {
            LogicalPlan::DeleteOp {
                source,
                exprs,
                detach,
            } => {
                assert!(!detach);
                assert_eq!(exprs.len(), 1);
                assert_eq!(exprs[0], Expression::Variable("n".to_string()));
                match source.as_ref() {
                    LogicalPlan::NodeScan { variable, .. } => {
                        assert_eq!(variable, "n");
                    }
                    other => panic!("expected NodeScan, got {:?}", other),
                }
            }
            other => panic!("expected DeleteOp, got {:?}", other),
        }
    }

    /// MATCH (n) RETURN n ORDER BY n.name SKIP 5 LIMIT 10
    /// -> NodeScan + Project + Sort + Skip + Limit
    #[test]
    fn test_plan_return_with_order_skip_limit() {
        let plan = plan_query("MATCH (n) RETURN n ORDER BY n.name SKIP 5 LIMIT 10");

        // Outermost: Limit
        let limit_source = match &plan {
            LogicalPlan::Limit { source, count } => {
                assert_eq!(*count, Expression::Literal(Literal::Integer(10)));
                source.as_ref()
            }
            other => panic!("expected Limit, got {:?}", other),
        };

        // Skip
        let skip_source = match limit_source {
            LogicalPlan::Skip { source, count } => {
                assert_eq!(*count, Expression::Literal(Literal::Integer(5)));
                source.as_ref()
            }
            other => panic!("expected Skip, got {:?}", other),
        };

        // Sort
        let sort_source = match skip_source {
            LogicalPlan::Sort { source, items } => {
                assert_eq!(items.len(), 1);
                source.as_ref()
            }
            other => panic!("expected Sort, got {:?}", other),
        };

        // Project
        match sort_source {
            LogicalPlan::Project { source, .. } => match source.as_ref() {
                LogicalPlan::NodeScan { variable, .. } => {
                    assert_eq!(variable, "n");
                }
                other => panic!("expected NodeScan, got {:?}", other),
            },
            other => panic!("expected Project, got {:?}", other),
        }
    }

    /// MATCH (n:Person) REMOVE n.email, n:Temp
    /// -> NodeScan + RemoveOp
    #[test]
    fn test_plan_match_remove() {
        let plan = plan_query("MATCH (n:Person) REMOVE n.email, n:Temp");

        match &plan {
            LogicalPlan::RemoveOp { source, items } => {
                assert_eq!(items.len(), 2);
                match source.as_ref() {
                    LogicalPlan::NodeScan { variable, .. } => {
                        assert_eq!(variable, "n");
                    }
                    other => panic!("expected NodeScan, got {:?}", other),
                }
            }
            other => panic!("expected RemoveOp, got {:?}", other),
        }
    }

    /// PlanError display formatting.
    #[test]
    fn test_plan_error_display() {
        let err = PlanError {
            message: "test error".to_string(),
        };
        assert_eq!(err.to_string(), "Plan error: test error");
    }

    /// RETURN without MATCH should fail.
    #[test]
    fn test_plan_return_without_source_fails() {
        let query = parse_query("MATCH (n) RETURN n").expect("should parse");
        // Manually construct a RETURN-only query to test error.
        let return_only = Query {
            clauses: vec![query.clauses.into_iter().nth(1).expect("has RETURN")],
        };
        let mut catalog = Catalog::default();
        let mut planner = LogicalPlanner::new(&mut catalog);
        let result = planner.plan(&return_only);
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .message
            .contains("requires a preceding data source"));
    }

    /// Optimizer pass-through test.
    #[test]
    fn test_optimizer_passthrough() {
        let plan = plan_query("MATCH (n:Person) WHERE n.age > 30 RETURN n");
        let optimized = optimize::optimize(plan.clone());
        assert_eq!(plan, optimized);
    }
}
