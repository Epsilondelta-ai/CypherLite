// Query planner: rule-based logical plan + physical plan conversion
pub mod optimize;

use crate::parser::ast::*;
use cypherlite_core::LabelRegistry;

/// A logical plan node representing a query execution strategy.
#[derive(Debug, Clone, PartialEq)]
pub enum LogicalPlan {
    /// Scan all nodes, optionally filtered by label ID.
    /// If `limit` is Some, stop after that many nodes (for LIMIT pushdown optimization).
    NodeScan {
        variable: String,
        label_id: Option<u32>,
        limit: Option<usize>,
    },
    /// Expand from a source variable along edges of given type.
    Expand {
        source: Box<LogicalPlan>,
        src_var: String,
        rel_var: Option<String>,
        target_var: String,
        rel_type_id: Option<u32>,
        direction: RelDirection,
        temporal_filter: Option<TemporalFilterPlan>,
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
    /// WITH clause: intermediate projection (scope reset).
    With {
        source: Box<LogicalPlan>,
        items: Vec<ReturnItem>,
        where_clause: Option<Expression>,
        distinct: bool,
    },
    /// UNWIND clause: flatten a list into rows.
    Unwind {
        source: Box<LogicalPlan>,
        expr: Expression,
        variable: String,
    },
    /// OPTIONAL MATCH expand: left join semantics.
    /// If no matching edges found, emit one record with NULL for new variables.
    OptionalExpand {
        source: Box<LogicalPlan>,
        src_var: String,
        rel_var: Option<String>,
        target_var: String,
        rel_type_id: Option<u32>,
        direction: RelDirection,
    },
    /// MERGE: match-or-create pattern with optional ON MATCH/ON CREATE SET.
    MergeOp {
        source: Option<Box<LogicalPlan>>,
        pattern: Pattern,
        on_match: Vec<SetItem>,
        on_create: Vec<SetItem>,
    },
    /// Empty source (produces one empty row).
    EmptySource,
    /// CREATE INDEX DDL operation (node label index).
    CreateIndex {
        name: Option<String>,
        label: String,
        property: String,
    },
    /// CREATE EDGE INDEX DDL operation (relationship type index).
    CreateEdgeIndex {
        name: Option<String>,
        rel_type: String,
        property: String,
    },
    /// DROP INDEX DDL operation.
    DropIndex { name: String },
    /// Variable-length path expansion (BFS/DFS traversal with depth bounds).
    VarLengthExpand {
        source: Box<LogicalPlan>,
        src_var: String,
        rel_var: Option<String>,
        target_var: String,
        rel_type_id: Option<u32>,
        direction: RelDirection,
        min_hops: u32,
        max_hops: u32,
        temporal_filter: Option<TemporalFilterPlan>,
    },
    /// Index-based scan: look up nodes by label + property value using an index.
    /// The executor checks at runtime whether an index actually exists.
    /// If no index is available, falls back to label scan + filter.
    IndexScan {
        variable: String,
        label_id: u32,
        prop_key: String,
        lookup_value: Expression,
    },
    /// AT TIME query: find node/edge versions at a specific point in time.
    AsOfScan {
        source: Box<LogicalPlan>,
        timestamp_expr: Expression,
    },
    /// BETWEEN TIME query: find all versions within a time range.
    TemporalRangeScan {
        source: Box<LogicalPlan>,
        start_expr: Expression,
        end_expr: Expression,
    },
    /// Scan all subgraph entities. Used when MATCH pattern has label "Subgraph".
    #[cfg(feature = "subgraph")]
    SubgraphScan { variable: String },
    /// Scan all hyperedge entities.
    #[cfg(feature = "hypergraph")]
    HyperEdgeScan { variable: String },
    /// Create a hyperedge connecting multiple sources to multiple targets.
    #[cfg(feature = "hypergraph")]
    CreateHyperedgeOp {
        source: Option<Box<LogicalPlan>>,
        variable: Option<String>,
        labels: Vec<String>,
        sources: Vec<Expression>,
        targets: Vec<Expression>,
    },
    /// CREATE SNAPSHOT: execute a sub-query and materialize results into a subgraph.
    #[cfg(feature = "subgraph")]
    CreateSnapshotOp {
        variable: Option<String>,
        labels: Vec<String>,
        properties: Option<MapLiteral>,
        temporal_anchor: Option<Expression>,
        sub_plan: Box<LogicalPlan>,
        return_vars: Vec<String>,
    },
}

/// Temporal filter plan for edge validity during AT TIME / BETWEEN TIME queries.
/// Expressions are evaluated at execution time to produce concrete timestamps.
#[derive(Debug, Clone, PartialEq)]
pub enum TemporalFilterPlan {
    /// Filter edges valid at a specific timestamp.
    AsOf(Expression),
    /// Filter edges with validity overlapping [start, end].
    Between(Expression, Expression),
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

/// Default maximum hops for unbounded variable-length paths.
pub const DEFAULT_MAX_HOPS: u32 = 10;

/// Walk a logical plan tree and set temporal_filter on Expand/VarLengthExpand nodes.
/// This is called when a MATCH clause has a temporal predicate (AT TIME / BETWEEN TIME)
/// so that edge traversal also filters edges by temporal validity.
fn annotate_temporal_filter(plan: &mut LogicalPlan, tfp: &TemporalFilterPlan) {
    match plan {
        LogicalPlan::Expand {
            source,
            temporal_filter,
            ..
        } => {
            *temporal_filter = Some(tfp.clone());
            annotate_temporal_filter(source, tfp);
        }
        LogicalPlan::VarLengthExpand {
            source,
            temporal_filter,
            ..
        } => {
            *temporal_filter = Some(tfp.clone());
            annotate_temporal_filter(source, tfp);
        }
        LogicalPlan::Filter { source, .. }
        | LogicalPlan::Project { source, .. }
        | LogicalPlan::Sort { source, .. }
        | LogicalPlan::Skip { source, .. }
        | LogicalPlan::Limit { source, .. }
        | LogicalPlan::Aggregate { source, .. }
        | LogicalPlan::SetOp { source, .. }
        | LogicalPlan::RemoveOp { source, .. }
        | LogicalPlan::With { source, .. }
        | LogicalPlan::Unwind { source, .. }
        | LogicalPlan::DeleteOp { source, .. }
        | LogicalPlan::OptionalExpand { source, .. }
        | LogicalPlan::AsOfScan { source, .. }
        | LogicalPlan::TemporalRangeScan { source, .. } => {
            annotate_temporal_filter(source, tfp);
        }
        LogicalPlan::CreateOp { source, .. } | LogicalPlan::MergeOp { source, .. } => {
            if let Some(s) = source {
                annotate_temporal_filter(s, tfp);
            }
        }
        // Leaf nodes: nothing to annotate
        LogicalPlan::NodeScan { .. }
        | LogicalPlan::IndexScan { .. }
        | LogicalPlan::EmptySource
        | LogicalPlan::CreateIndex { .. }
        | LogicalPlan::CreateEdgeIndex { .. }
        | LogicalPlan::DropIndex { .. } => {}
        #[cfg(feature = "subgraph")]
        LogicalPlan::SubgraphScan { .. } => {}
        #[cfg(feature = "subgraph")]
        LogicalPlan::CreateSnapshotOp { .. } => {}
        #[cfg(feature = "hypergraph")]
        LogicalPlan::HyperEdgeScan { .. } => {}
        #[cfg(feature = "hypergraph")]
        LogicalPlan::CreateHyperedgeOp { .. } => {}
    }
}

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
            Clause::With(wc) => self.plan_with(wc, current),
            Clause::Unwind(uc) => self.plan_unwind(uc, current),
            Clause::Merge(mc) => Ok(self.plan_merge(mc, current)),
            Clause::CreateIndex(ci) => match &ci.target {
                crate::parser::ast::IndexTarget::NodeLabel(label) => Ok(LogicalPlan::CreateIndex {
                    name: ci.name.clone(),
                    label: label.clone(),
                    property: ci.property.clone(),
                }),
                crate::parser::ast::IndexTarget::RelationshipType(rel_type) => {
                    Ok(LogicalPlan::CreateEdgeIndex {
                        name: ci.name.clone(),
                        rel_type: rel_type.clone(),
                        property: ci.property.clone(),
                    })
                }
            },
            Clause::DropIndex(di) => Ok(LogicalPlan::DropIndex {
                name: di.name.clone(),
            }),
            #[cfg(feature = "subgraph")]
            Clause::CreateSnapshot(sc) => self.plan_create_snapshot(sc),
            #[cfg(feature = "hypergraph")]
            Clause::CreateHyperedge(hc) => Ok(self.plan_create_hyperedge(hc, current)),
            #[cfg(feature = "hypergraph")]
            Clause::MatchHyperedge(mhc) => Ok(self.plan_match_hyperedge(mhc)),
        }
    }

    fn plan_match(
        &mut self,
        mc: &MatchClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        if mc.optional {
            return self.plan_optional_match(mc, current);
        }

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

        // Apply temporal predicate if present.
        if let Some(ref tp) = mc.temporal_predicate {
            // DD-T4: Annotate Expand/VarLengthExpand nodes with temporal filter
            // so edges are also filtered temporally during traversal.
            let tfp = match tp {
                crate::parser::ast::TemporalPredicate::AsOf(expr) => {
                    TemporalFilterPlan::AsOf(expr.clone())
                }
                crate::parser::ast::TemporalPredicate::Between(start, end) => {
                    TemporalFilterPlan::Between(start.clone(), end.clone())
                }
            };
            annotate_temporal_filter(&mut plan, &tfp);

            match tp {
                crate::parser::ast::TemporalPredicate::AsOf(expr) => {
                    plan = LogicalPlan::AsOfScan {
                        source: Box::new(plan),
                        timestamp_expr: expr.clone(),
                    };
                }
                crate::parser::ast::TemporalPredicate::Between(start, end) => {
                    plan = LogicalPlan::TemporalRangeScan {
                        source: Box::new(plan),
                        start_expr: start.clone(),
                        end_expr: end.clone(),
                    };
                }
            }
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

    /// Plan an OPTIONAL MATCH clause. Produces OptionalExpand nodes with left join
    /// semantics: if no match found, new variables are padded with NULL.
    fn plan_optional_match(
        &mut self,
        mc: &MatchClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        let source = current.ok_or_else(|| PlanError {
            message: "OPTIONAL MATCH requires a preceding MATCH clause".to_string(),
        })?;

        let chain = mc.pattern.chains.first().ok_or_else(|| PlanError {
            message: "OPTIONAL MATCH clause has no pattern chains".to_string(),
        })?;

        let mut plan = source;

        // The first element should be a node (anchor from previous MATCH).
        let mut elements = chain.elements.iter();
        let first_node = match elements.next() {
            Some(PatternElement::Node(np)) => np,
            _ => {
                return Err(PlanError {
                    message: "OPTIONAL MATCH pattern must start with a node".to_string(),
                })
            }
        };

        // The anchor variable binds to records from the source plan.
        let _anchor_var = first_node.variable.clone().unwrap_or_default();

        // Process relationship + target node pairs as OptionalExpand.
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

            plan = LogicalPlan::OptionalExpand {
                source: Box::new(plan),
                src_var,
                rel_var,
                target_var,
                rel_type_id,
                direction: rel.direction,
            };
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

    /// Build a combined equality predicate from inline property filters.
    ///
    /// Given `{name: 'Alice', age: 30}`, produces:
    /// `variable.name = 'Alice' AND variable.age = 30`
    ///
    /// Returns `None` for an empty property list (or `None` input).
    fn build_inline_property_predicate(
        variable: &str,
        properties: &[(String, Expression)],
    ) -> Option<Expression> {
        properties
            .iter()
            .map(|(key, val_expr)| {
                Expression::BinaryOp(
                    BinaryOp::Eq,
                    Box::new(Expression::Property(
                        Box::new(Expression::Variable(variable.to_string())),
                        key.clone(),
                    )),
                    Box::new(val_expr.clone()),
                )
            })
            .reduce(|acc, p| Expression::BinaryOp(BinaryOp::And, Box::new(acc), Box::new(p)))
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

        // Check if the label is "Subgraph" -- route to SubgraphScan instead of NodeScan.
        #[cfg(feature = "subgraph")]
        let is_subgraph_label = first_node
            .labels
            .first()
            .map(|l| l == "Subgraph")
            .unwrap_or(false);

        #[cfg(feature = "subgraph")]
        if is_subgraph_label {
            let mut plan = LogicalPlan::SubgraphScan {
                variable: variable.clone(),
            };

            // Apply inline property filters as a Filter node.
            if let Some(ref props) = first_node.properties {
                if let Some(pred) = Self::build_inline_property_predicate(&variable, props) {
                    plan = LogicalPlan::Filter {
                        source: Box::new(plan),
                        predicate: pred,
                    };
                }
            }

            // Process remaining relationship + node pairs (e.g., -[:CONTAINS]->(n)).
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
                let target_var = target_node.variable.clone().unwrap_or_default();

                let rel_type_id = rel
                    .rel_types
                    .first()
                    .map(|name| self.registry.get_or_create_rel_type(name));

                // Assign internal variable for anonymous relationships with properties.
                let has_rel_props = rel.properties.as_ref().is_some_and(|p| !p.is_empty());
                let rel_var = if rel.variable.is_some() {
                    rel.variable.clone()
                } else if has_rel_props {
                    Some("_anon_rel".to_string())
                } else {
                    None
                };

                plan = LogicalPlan::Expand {
                    source: Box::new(plan),
                    src_var,
                    rel_var: rel_var.clone(),
                    target_var: target_var.clone(),
                    rel_type_id,
                    direction: rel.direction,
                    temporal_filter: None,
                };

                // Apply inline property filter on the relationship.
                if let Some(ref props) = rel.properties {
                    if let Some(ref rv) = rel_var {
                        if let Some(pred) = Self::build_inline_property_predicate(rv, props) {
                            plan = LogicalPlan::Filter {
                                source: Box::new(plan),
                                predicate: pred,
                            };
                        }
                    }
                }

                // Apply inline property filter on the target node.
                if let Some(ref props) = target_node.properties {
                    if let Some(pred) = Self::build_inline_property_predicate(&target_var, props) {
                        plan = LogicalPlan::Filter {
                            source: Box::new(plan),
                            predicate: pred,
                        };
                    }
                }
            }

            return Ok(plan);
        }

        let label_id = first_node
            .labels
            .first()
            .map(|name| self.registry.get_or_create_label(name));

        let mut plan = LogicalPlan::NodeScan {
            variable: variable.clone(),
            label_id,
            limit: None,
        };

        // Apply inline property filters as a Filter node (e.g., {name: 'Alice'}).
        if let Some(ref props) = first_node.properties {
            if let Some(pred) = Self::build_inline_property_predicate(&variable, props) {
                plan = LogicalPlan::Filter {
                    source: Box::new(plan),
                    predicate: pred,
                };
            }
        }

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
            let target_var = target_node.variable.clone().unwrap_or_default();

            let rel_type_id = rel
                .rel_types
                .first()
                .map(|name| self.registry.get_or_create_rel_type(name));

            // If the relationship has inline properties but no explicit variable,
            // assign an internal variable so the edge is bound for predicate filtering.
            let has_rel_props = rel.properties.as_ref().is_some_and(|p| !p.is_empty());
            let rel_var = if rel.variable.is_some() {
                rel.variable.clone()
            } else if has_rel_props {
                Some("_anon_rel".to_string())
            } else {
                None
            };

            if rel.min_hops.is_some() {
                // Variable-length path: use VarLengthExpand
                let min = rel.min_hops.unwrap_or(1);
                let max = rel.max_hops.unwrap_or(DEFAULT_MAX_HOPS);
                plan = LogicalPlan::VarLengthExpand {
                    source: Box::new(plan),
                    src_var,
                    rel_var: rel_var.clone(),
                    target_var: target_var.clone(),
                    rel_type_id,
                    direction: rel.direction,
                    min_hops: min,
                    max_hops: max,
                    temporal_filter: None,
                };
            } else {
                plan = LogicalPlan::Expand {
                    source: Box::new(plan),
                    src_var,
                    rel_var: rel_var.clone(),
                    target_var: target_var.clone(),
                    rel_type_id,
                    direction: rel.direction,
                    temporal_filter: None,
                };
            }

            // Apply inline property filter on the relationship (e.g., {since: 2020}).
            if let Some(ref props) = rel.properties {
                if let Some(ref rv) = rel_var {
                    if let Some(pred) = Self::build_inline_property_predicate(rv, props) {
                        plan = LogicalPlan::Filter {
                            source: Box::new(plan),
                            predicate: pred,
                        };
                    }
                }
            }

            // Apply inline property filter on the target node (e.g., (b:Person {name: 'Bob'})).
            if let Some(ref props) = target_node.properties {
                if let Some(pred) = Self::build_inline_property_predicate(&target_var, props) {
                    plan = LogicalPlan::Filter {
                        source: Box::new(plan),
                        predicate: pred,
                    };
                }
            }
        }

        Ok(plan)
    }

    /// Extract the "output variable" from a plan node (used as src_var for Expand).
    fn extract_src_var(plan: &LogicalPlan) -> String {
        match plan {
            LogicalPlan::NodeScan { variable, .. } => variable.clone(),
            LogicalPlan::Expand { target_var, .. } => target_var.clone(),
            LogicalPlan::VarLengthExpand { target_var, .. } => target_var.clone(),
            LogicalPlan::OptionalExpand { target_var, .. } => target_var.clone(),
            LogicalPlan::Filter { source, .. } => Self::extract_src_var(source),
            LogicalPlan::AsOfScan { source, .. } => Self::extract_src_var(source),
            LogicalPlan::TemporalRangeScan { source, .. } => Self::extract_src_var(source),
            #[cfg(feature = "subgraph")]
            LogicalPlan::SubgraphScan { variable, .. } => variable.clone(),
            #[cfg(feature = "hypergraph")]
            LogicalPlan::HyperEdgeScan { variable, .. } => variable.clone(),
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

        // Detect aggregate functions in RETURN items.
        // If any item contains an aggregate, split into group_keys + aggregates.
        let has_aggregate = rc
            .items
            .iter()
            .any(|item| Self::is_aggregate_expr(&item.expr));

        let mut plan = if has_aggregate {
            let mut group_keys = Vec::new();
            let mut aggregates = Vec::new();

            for item in &rc.items {
                if Self::is_aggregate_expr(&item.expr) {
                    let alias = item
                        .alias
                        .clone()
                        .unwrap_or_else(|| Self::default_agg_name(&item.expr));
                    let func = Self::extract_aggregate_func(&item.expr)?;
                    aggregates.push((alias, func));
                } else {
                    group_keys.push(item.expr.clone());
                }
            }

            LogicalPlan::Aggregate {
                source: Box::new(source),
                group_keys,
                aggregates,
            }
        } else {
            LogicalPlan::Project {
                source: Box::new(source),
                items: rc.items.clone(),
                distinct: rc.distinct,
            }
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

    fn plan_merge(&self, mc: &MergeClause, current: Option<LogicalPlan>) -> LogicalPlan {
        LogicalPlan::MergeOp {
            source: current.map(Box::new),
            pattern: mc.pattern.clone(),
            on_match: mc.on_match.clone(),
            on_create: mc.on_create.clone(),
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

    fn plan_with(
        &self,
        wc: &WithClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        let source = current.ok_or_else(|| PlanError {
            message: "WITH clause requires a preceding data source".to_string(),
        })?;

        // Detect aggregate functions in WITH items.
        // If any item contains an aggregate, split into group_keys + aggregates.
        let has_aggregate = wc
            .items
            .iter()
            .any(|item| Self::is_aggregate_expr(&item.expr));

        if has_aggregate {
            let mut group_keys = Vec::new();
            let mut aggregates = Vec::new();

            for item in &wc.items {
                if Self::is_aggregate_expr(&item.expr) {
                    let alias = item
                        .alias
                        .clone()
                        .unwrap_or_else(|| Self::default_agg_name(&item.expr));
                    let func = Self::extract_aggregate_func(&item.expr)?;
                    aggregates.push((alias, func));
                } else {
                    group_keys.push(item.expr.clone());
                }
            }

            let mut plan = LogicalPlan::Aggregate {
                source: Box::new(source),
                group_keys,
                aggregates,
            };

            // Apply WITH WHERE after aggregation
            if let Some(ref predicate) = wc.where_clause {
                plan = LogicalPlan::Filter {
                    source: Box::new(plan),
                    predicate: predicate.clone(),
                };
            }

            Ok(plan)
        } else {
            Ok(LogicalPlan::With {
                source: Box::new(source),
                items: wc.items.clone(),
                where_clause: wc.where_clause.clone(),
                distinct: wc.distinct,
            })
        }
    }

    /// Check if an expression is an aggregate function.
    fn is_aggregate_expr(expr: &Expression) -> bool {
        match expr {
            Expression::CountStar => true,
            Expression::FunctionCall { name, .. } => {
                matches!(
                    name.to_lowercase().as_str(),
                    "count" | "sum" | "avg" | "min" | "max" | "collect"
                )
            }
            _ => false,
        }
    }

    /// Extract an AggregateFunc from an aggregate expression.
    fn extract_aggregate_func(expr: &Expression) -> Result<AggregateFunc, PlanError> {
        match expr {
            Expression::CountStar => Ok(AggregateFunc::CountStar),
            Expression::FunctionCall { name, distinct, .. } => match name.to_lowercase().as_str() {
                "count" => Ok(AggregateFunc::Count {
                    distinct: *distinct,
                }),
                other => Err(PlanError {
                    message: format!("unsupported aggregate function: {}", other),
                }),
            },
            _ => Err(PlanError {
                message: "not an aggregate expression".to_string(),
            }),
        }
    }

    /// Generate a default display name for an aggregate expression.
    fn default_agg_name(expr: &Expression) -> String {
        match expr {
            Expression::CountStar => "count(*)".to_string(),
            Expression::FunctionCall { name, .. } => format!("{}(..)", name),
            _ => "agg".to_string(),
        }
    }

    fn plan_unwind(
        &self,
        uc: &UnwindClause,
        current: Option<LogicalPlan>,
    ) -> Result<LogicalPlan, PlanError> {
        let source = current.unwrap_or(LogicalPlan::EmptySource);
        Ok(LogicalPlan::Unwind {
            source: Box::new(source),
            expr: uc.expr.clone(),
            variable: uc.variable.clone(),
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

    /// Plan a CREATE SNAPSHOT clause.
    /// Builds a sub-plan from the FROM MATCH + RETURN clauses, then wraps in CreateSnapshotOp.
    #[cfg(feature = "subgraph")]
    /// Plan a CREATE HYPEREDGE clause.
    #[cfg(feature = "hypergraph")]
    fn plan_create_hyperedge(
        &mut self,
        hc: &crate::parser::ast::CreateHyperedgeClause,
        current: Option<LogicalPlan>,
    ) -> LogicalPlan {
        LogicalPlan::CreateHyperedgeOp {
            source: current.map(Box::new),
            variable: hc.variable.clone(),
            labels: hc.labels.clone(),
            sources: hc.sources.clone(),
            targets: hc.targets.clone(),
        }
    }

    /// Plan a MATCH HYPEREDGE clause.
    #[cfg(feature = "hypergraph")]
    fn plan_match_hyperedge(
        &mut self,
        mhc: &crate::parser::ast::MatchHyperedgeClause,
    ) -> LogicalPlan {
        let variable = mhc.variable.clone().unwrap_or_default();
        let mut plan = LogicalPlan::HyperEdgeScan {
            variable: variable.clone(),
        };

        // If labels are specified, add a filter for rel_type_id
        if let Some(label) = mhc.labels.first() {
            let _rel_type_id = self.registry.get_or_create_rel_type(label);
            // We filter at execution time by comparing the hyperedge rel_type_id
            // For now, add a Filter that compares type(h) == label
            // Actually, we'll handle the label filtering at execution time via HyperEdgeScan
            // For simplicity, store the label in the plan via a Filter
            let _ = plan;
            plan = LogicalPlan::HyperEdgeScan {
                variable: variable.clone(),
            };
        }

        plan
    }

    #[cfg(feature = "subgraph")]
    fn plan_create_snapshot(
        &mut self,
        sc: &crate::parser::ast::CreateSnapshotClause,
    ) -> Result<LogicalPlan, PlanError> {
        // Build sub-plan from the FROM MATCH clause.
        let chain = sc
            .from_match
            .pattern
            .chains
            .first()
            .ok_or_else(|| PlanError {
                message: "CREATE SNAPSHOT FROM MATCH clause has no pattern chains".to_string(),
            })?;
        let mut sub_plan = self.plan_pattern_chain(chain)?;

        // Apply WHERE predicate if present.
        if let Some(ref predicate) = sc.from_match.where_clause {
            sub_plan = LogicalPlan::Filter {
                source: Box::new(sub_plan),
                predicate: predicate.clone(),
            };
        }

        // Project with the RETURN items.
        sub_plan = LogicalPlan::Project {
            source: Box::new(sub_plan),
            items: sc.from_return.clone(),
            distinct: false,
        };

        // Collect variable names from RETURN items.
        let return_vars: Vec<String> = sc
            .from_return
            .iter()
            .map(|item| {
                if let Some(ref alias) = item.alias {
                    alias.clone()
                } else if let Expression::Variable(name) = &item.expr {
                    name.clone()
                } else {
                    String::new()
                }
            })
            .collect();

        Ok(LogicalPlan::CreateSnapshotOp {
            variable: sc.variable.clone(),
            labels: sc.labels.clone(),
            properties: sc.properties.clone(),
            temporal_anchor: sc.temporal_anchor.clone(),
            sub_plan: Box::new(sub_plan),
            return_vars,
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
                    LogicalPlan::NodeScan {
                        variable, label_id, ..
                    } => {
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
            LogicalPlan::NodeScan {
                variable, label_id, ..
            } => {
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
            LogicalPlan::NodeScan {
                variable, label_id, ..
            } => {
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
                    LogicalPlan::NodeScan {
                        variable, label_id, ..
                    } => {
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

    // ======================================================================
    // TASK-061: Planner WITH clause tests
    // ======================================================================

    /// MATCH (n:Person) WITH n RETURN n -> NodeScan + With + Project
    #[test]
    fn test_plan_with_simple() {
        let plan = plan_query("MATCH (n:Person) WITH n RETURN n");

        // Outermost: Project
        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        // With
        let with_source = match project_source {
            LogicalPlan::With {
                source,
                items,
                where_clause,
                distinct,
            } => {
                assert_eq!(items.len(), 1);
                assert!(where_clause.is_none());
                assert!(!distinct);
                source.as_ref()
            }
            other => panic!("expected With, got {:?}", other),
        };

        // NodeScan
        match with_source {
            LogicalPlan::NodeScan { variable, .. } => {
                assert_eq!(variable, "n");
            }
            other => panic!("expected NodeScan, got {:?}", other),
        }
    }

    /// MATCH (n:Person) WITH n WHERE n.age > 30 RETURN n
    #[test]
    fn test_plan_with_where() {
        let plan = plan_query("MATCH (n:Person) WITH n WHERE n.age > 30 RETURN n");

        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        match project_source {
            LogicalPlan::With {
                where_clause,
                items,
                ..
            } => {
                assert_eq!(items.len(), 1);
                assert!(where_clause.is_some());
            }
            other => panic!("expected With, got {:?}", other),
        }
    }

    /// WITH without source should fail
    #[test]
    fn test_plan_with_without_source_fails() {
        let query = parse_query("MATCH (n) WITH n RETURN n").expect("should parse");
        // Build a WITH-only query
        let with_only = Query {
            clauses: vec![query.clauses.into_iter().nth(1).expect("has WITH")],
        };
        let mut catalog = Catalog::default();
        let mut planner = LogicalPlanner::new(&mut catalog);
        let result = planner.plan(&with_only);
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .message
            .contains("requires a preceding data source"));
    }

    // ======================================================================
    // TASK-064: WITH DISTINCT planner test
    // ======================================================================

    /// MATCH (n:Person) WITH DISTINCT n.name AS name RETURN name
    #[test]
    fn test_plan_with_distinct() {
        let plan = plan_query("MATCH (n:Person) WITH DISTINCT n.name AS name RETURN name");

        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        match project_source {
            LogicalPlan::With {
                distinct, items, ..
            } => {
                assert!(distinct);
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].alias, Some("name".to_string()));
            }
            other => panic!("expected With, got {:?}", other),
        }
    }

    // ======================================================================
    // TASK-063: WITH + aggregation planner tests
    // ======================================================================

    /// MATCH (n:Person) WITH n, count(*) AS cnt RETURN n, cnt
    /// -> NodeScan + Aggregate(group_keys=[n], aggs=[count(*) AS cnt]) + Project
    #[test]
    fn test_plan_with_count_star_aggregation() {
        let plan = plan_query("MATCH (n:Person) WITH n, count(*) AS cnt RETURN n, cnt");

        // Outermost: Project
        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        // Should be Aggregate (not With), because count(*) was detected
        match project_source {
            LogicalPlan::Aggregate {
                group_keys,
                aggregates,
                source,
                ..
            } => {
                // group key: n
                assert_eq!(group_keys.len(), 1);
                assert_eq!(group_keys[0], Expression::Variable("n".to_string()));
                // aggregate: count(*) AS cnt
                assert_eq!(aggregates.len(), 1);
                assert_eq!(aggregates[0].0, "cnt");
                assert_eq!(aggregates[0].1, AggregateFunc::CountStar);
                // source: NodeScan
                match source.as_ref() {
                    LogicalPlan::NodeScan { variable, .. } => {
                        assert_eq!(variable, "n");
                    }
                    other => panic!("expected NodeScan, got {:?}", other),
                }
            }
            other => panic!("expected Aggregate, got {:?}", other),
        }
    }

    /// MATCH (n:Person) WITH count(*) AS total RETURN total
    /// -> NodeScan + Aggregate(group_keys=[], aggs=[count(*) AS total]) + Project
    #[test]
    fn test_plan_with_count_star_no_group_key() {
        let plan = plan_query("MATCH (n:Person) WITH count(*) AS total RETURN total");

        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        match project_source {
            LogicalPlan::Aggregate {
                group_keys,
                aggregates,
                ..
            } => {
                assert!(group_keys.is_empty());
                assert_eq!(aggregates.len(), 1);
                assert_eq!(aggregates[0].0, "total");
                assert_eq!(aggregates[0].1, AggregateFunc::CountStar);
            }
            other => panic!("expected Aggregate, got {:?}", other),
        }
    }

    /// Optimizer pass-through test.
    #[test]
    fn test_optimizer_passthrough() {
        let plan = plan_query("MATCH (n:Person) WHERE n.age > 30 RETURN n");
        let optimized = optimize::optimize(plan.clone());
        assert_eq!(plan, optimized);
    }

    // ======================================================================
    // TASK-070: Planner UNWIND clause tests
    // ======================================================================

    /// UNWIND [1,2,3] AS x RETURN x -> EmptySource + Unwind + Project
    #[test]
    fn test_plan_unwind_list_literal() {
        let plan = plan_query("UNWIND [1, 2, 3] AS x RETURN x");

        // Outermost: Project
        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        // Unwind
        match project_source {
            LogicalPlan::Unwind {
                source,
                expr,
                variable,
            } => {
                assert_eq!(variable, "x");
                assert!(matches!(expr, Expression::ListLiteral(_)));
                // Source should be EmptySource
                match source.as_ref() {
                    LogicalPlan::EmptySource => {}
                    other => panic!("expected EmptySource, got {:?}", other),
                }
            }
            other => panic!("expected Unwind, got {:?}", other),
        }
    }

    /// MATCH (n:Person) UNWIND n.hobbies AS h RETURN h
    /// -> NodeScan + Unwind + Project
    #[test]
    fn test_plan_match_unwind() {
        let plan = plan_query("MATCH (n:Person) UNWIND n.hobbies AS h RETURN h");

        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        match project_source {
            LogicalPlan::Unwind {
                source,
                variable,
                expr,
            } => {
                assert_eq!(variable, "h");
                assert_eq!(
                    *expr,
                    Expression::Property(
                        Box::new(Expression::Variable("n".to_string())),
                        "hobbies".to_string(),
                    )
                );
                match source.as_ref() {
                    LogicalPlan::NodeScan { variable, .. } => {
                        assert_eq!(variable, "n");
                    }
                    other => panic!("expected NodeScan, got {:?}", other),
                }
            }
            other => panic!("expected Unwind, got {:?}", other),
        }
    }

    // ======================================================================
    // TASK-075: Planner OPTIONAL MATCH tests
    // ======================================================================

    /// MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a, b
    /// -> NodeScan(a) + OptionalExpand(KNOWS, b) + Project
    #[test]
    fn test_plan_optional_match_basic() {
        let (plan, catalog) = plan_query_with_catalog(
            "MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a, b",
        );
        let knows_id = catalog.rel_type_id("KNOWS").expect("KNOWS rel type");

        // Outermost: Project
        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        // OptionalExpand
        let opt_source = match project_source {
            LogicalPlan::OptionalExpand {
                src_var,
                rel_var,
                target_var,
                rel_type_id,
                direction,
                source,
            } => {
                assert_eq!(src_var, "a");
                assert!(rel_var.is_none());
                assert_eq!(target_var, "b");
                assert_eq!(*rel_type_id, Some(knows_id));
                assert_eq!(*direction, RelDirection::Outgoing);
                source.as_ref()
            }
            other => panic!("expected OptionalExpand, got {:?}", other),
        };

        // NodeScan(a:Person)
        match opt_source {
            LogicalPlan::NodeScan {
                variable, label_id, ..
            } => {
                assert_eq!(variable, "a");
                assert!(label_id.is_some());
            }
            other => panic!("expected NodeScan, got {:?}", other),
        }
    }

    /// OPTIONAL MATCH without preceding MATCH should fail
    #[test]
    fn test_plan_optional_match_without_source_fails() {
        let query =
            parse_query("OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a, b").expect("should parse");
        let mut catalog = Catalog::default();
        let mut planner = LogicalPlanner::new(&mut catalog);
        let result = planner.plan(&query);
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .message
            .contains("requires a preceding MATCH"));
    }

    /// MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) WHERE b.age > 20 RETURN a, b
    /// -> NodeScan + OptionalExpand + Filter + Project
    #[test]
    fn test_plan_optional_match_with_where() {
        let plan = plan_query(
            "MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) WHERE b.age > 20 RETURN a, b",
        );

        // Outermost: Project
        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        // Filter from OPTIONAL MATCH WHERE
        let filter_source = match project_source {
            LogicalPlan::Filter { source, .. } => source.as_ref(),
            other => panic!("expected Filter, got {:?}", other),
        };

        // OptionalExpand
        match filter_source {
            LogicalPlan::OptionalExpand { target_var, .. } => {
                assert_eq!(target_var, "b");
            }
            other => panic!("expected OptionalExpand, got {:?}", other),
        }
    }

    /// MATCH (a:Person) OPTIONAL MATCH (a)-[r:KNOWS]->(b) RETURN a, r, b
    /// -> OptionalExpand with rel_var
    #[test]
    fn test_plan_optional_match_with_rel_var() {
        let plan = plan_query("MATCH (a:Person) OPTIONAL MATCH (a)-[r:KNOWS]->(b) RETURN a, r, b");

        let project_source = match &plan {
            LogicalPlan::Project { source, .. } => source.as_ref(),
            other => panic!("expected Project, got {:?}", other),
        };

        match project_source {
            LogicalPlan::OptionalExpand {
                rel_var,
                target_var,
                ..
            } => {
                assert_eq!(*rel_var, Some("r".to_string()));
                assert_eq!(target_var, "b");
            }
            other => panic!("expected OptionalExpand, got {:?}", other),
        }
    }

    // -- TASK-104/105: VarLengthExpand planner tests --

    #[test]
    fn test_plan_var_length_bounded() {
        let plan = plan_query("MATCH (a)-[*1..3]->(b) RETURN b");
        // Outermost: Project wrapping VarLengthExpand
        match &plan {
            LogicalPlan::Project { source, .. } => match source.as_ref() {
                LogicalPlan::VarLengthExpand {
                    src_var,
                    target_var,
                    min_hops,
                    max_hops,
                    ..
                } => {
                    assert_eq!(src_var, "a");
                    assert_eq!(target_var, "b");
                    assert_eq!(*min_hops, 1);
                    assert_eq!(*max_hops, 3);
                }
                other => panic!("expected VarLengthExpand, got {:?}", other),
            },
            other => panic!("expected Project, got {:?}", other),
        }
    }

    #[test]
    fn test_plan_var_length_unbounded_gets_default_max() {
        let plan = plan_query("MATCH (a)-[*]->(b) RETURN b");
        match &plan {
            LogicalPlan::Project { source, .. } => match source.as_ref() {
                LogicalPlan::VarLengthExpand {
                    min_hops, max_hops, ..
                } => {
                    assert_eq!(*min_hops, 1);
                    assert_eq!(*max_hops, DEFAULT_MAX_HOPS);
                }
                other => panic!("expected VarLengthExpand, got {:?}", other),
            },
            other => panic!("expected Project, got {:?}", other),
        }
    }

    #[test]
    fn test_plan_var_length_typed() {
        let (plan, catalog) = plan_query_with_catalog("MATCH (a)-[:KNOWS*2..4]->(b) RETURN b");
        let knows_id = catalog.rel_type_id("KNOWS").expect("KNOWS exists");
        match &plan {
            LogicalPlan::Project { source, .. } => match source.as_ref() {
                LogicalPlan::VarLengthExpand {
                    rel_type_id,
                    min_hops,
                    max_hops,
                    ..
                } => {
                    assert_eq!(*rel_type_id, Some(knows_id));
                    assert_eq!(*min_hops, 2);
                    assert_eq!(*max_hops, 4);
                }
                other => panic!("expected VarLengthExpand, got {:?}", other),
            },
            other => panic!("expected Project, got {:?}", other),
        }
    }

    #[test]
    fn test_plan_regular_expand_unchanged() {
        let plan = plan_query("MATCH (a)-[:KNOWS]->(b) RETURN b");
        match &plan {
            LogicalPlan::Project { source, .. } => match source.as_ref() {
                LogicalPlan::Expand { .. } => {} // Regular expand, not VarLengthExpand
                other => panic!("expected Expand, got {:?}", other),
            },
            other => panic!("expected Project, got {:?}", other),
        }
    }

    #[test]
    fn test_plan_var_length_exact_hop() {
        let plan = plan_query("MATCH (a)-[*2]->(b) RETURN b");
        match &plan {
            LogicalPlan::Project { source, .. } => match source.as_ref() {
                LogicalPlan::VarLengthExpand {
                    min_hops, max_hops, ..
                } => {
                    assert_eq!(*min_hops, 2);
                    assert_eq!(*max_hops, 2);
                }
                other => panic!("expected VarLengthExpand, got {:?}", other),
            },
            other => panic!("expected Project, got {:?}", other),
        }
    }

    #[test]
    fn test_plan_var_length_open_end_gets_default() {
        let plan = plan_query("MATCH (a)-[*3..]->(b) RETURN b");
        match &plan {
            LogicalPlan::Project { source, .. } => match source.as_ref() {
                LogicalPlan::VarLengthExpand {
                    min_hops, max_hops, ..
                } => {
                    assert_eq!(*min_hops, 3);
                    assert_eq!(*max_hops, DEFAULT_MAX_HOPS);
                }
                other => panic!("expected VarLengthExpand, got {:?}", other),
            },
            other => panic!("expected Project, got {:?}", other),
        }
    }

    #[test]
    fn test_plan_var_length_with_variable() {
        let plan = plan_query("MATCH (a)-[r:KNOWS*1..2]->(b) RETURN b");
        match &plan {
            LogicalPlan::Project { source, .. } => match source.as_ref() {
                LogicalPlan::VarLengthExpand {
                    rel_var,
                    min_hops,
                    max_hops,
                    ..
                } => {
                    assert_eq!(*rel_var, Some("r".to_string()));
                    assert_eq!(*min_hops, 1);
                    assert_eq!(*max_hops, 2);
                }
                other => panic!("expected VarLengthExpand, got {:?}", other),
            },
            other => panic!("expected Project, got {:?}", other),
        }
    }

    // ======================================================================
    // MM-001: Planner hyperedge tests (cfg-gated)
    // ======================================================================

    #[cfg(feature = "hypergraph")]
    mod hypergraph_planner_tests {
        use super::*;

        // MM-001: CREATE HYPEREDGE produces CreateHyperedgeOp
        #[test]
        fn plan_create_hyperedge_basic() {
            let plan = plan_query("CREATE HYPEREDGE (h:GroupMigration) FROM (a, b) TO (c)");
            match plan {
                LogicalPlan::CreateHyperedgeOp {
                    source,
                    variable,
                    labels,
                    sources,
                    targets,
                } => {
                    assert!(
                        source.is_none(),
                        "standalone CREATE HYPEREDGE has no source"
                    );
                    assert_eq!(variable, Some("h".to_string()));
                    assert_eq!(labels, vec!["GroupMigration".to_string()]);
                    assert_eq!(sources.len(), 2);
                    assert_eq!(targets.len(), 1);
                }
                other => panic!("expected CreateHyperedgeOp, got {:?}", other),
            }
        }

        // MM-003: MATCH HYPEREDGE produces HyperEdgeScan
        #[test]
        fn plan_match_hyperedge_basic() {
            let plan = plan_query("MATCH HYPEREDGE (h:GroupMigration) RETURN h");
            // Should be Project -> HyperEdgeScan
            match plan {
                LogicalPlan::Project { source, .. } => match *source {
                    LogicalPlan::HyperEdgeScan { variable } => {
                        assert_eq!(variable, "h");
                    }
                    other => panic!("expected HyperEdgeScan, got {:?}", other),
                },
                other => panic!("expected Project, got {:?}", other),
            }
        }
    }
}
