// Semantic analysis: variable scope validation, label/type resolution

use crate::parser::ast::{
    Clause, CreateClause, DeleteClause, Expression, MatchClause, MergeClause, NodePattern, Pattern,
    PatternElement, RelationshipPattern, RemoveItem, ReturnClause, SetItem, WithClause,
};
use cypherlite_core::LabelRegistry;

pub mod symbol_table;
use symbol_table::{SymbolTable, VariableKind};

/// Semantic errors found during analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticError {
    pub message: String,
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Semantic error: {}", self.message)
    }
}

impl std::error::Error for SemanticError {}

/// Walks the AST to validate variable scoping and resolve names via a `LabelRegistry`.
pub struct SemanticAnalyzer<'a> {
    registry: &'a mut dyn LabelRegistry,
    symbols: SymbolTable,
}

impl<'a> SemanticAnalyzer<'a> {
    /// Create a new analyzer backed by the given registry.
    pub fn new(registry: &'a mut dyn LabelRegistry) -> Self {
        Self {
            registry,
            symbols: SymbolTable::new(),
        }
    }

    // @MX:ANCHOR: [AUTO] Central semantic validation — called by CypherLite API and Planner
    // @MX:REASON: fan_in >= 3; validates all queries before execution
    /// Analyze a query, resolving names and checking variable scoping.
    /// Returns the symbol table on success.
    pub fn analyze(
        &mut self,
        query: &crate::parser::ast::Query,
    ) -> Result<SymbolTable, SemanticError> {
        for clause in &query.clauses {
            self.analyze_clause(clause)?;
        }
        Ok(self.symbols.clone())
    }

    fn analyze_clause(&mut self, clause: &Clause) -> Result<(), SemanticError> {
        match clause {
            Clause::Match(m) => self.analyze_match(m),
            Clause::Create(c) => self.analyze_create(c),
            Clause::Merge(m) => self.analyze_merge(m),
            Clause::Return(r) => self.analyze_return(r),
            Clause::With(w) => self.analyze_with(w),
            Clause::Set(s) => self.analyze_set(s),
            Clause::Delete(d) => self.analyze_delete(d),
            Clause::Remove(r) => self.analyze_remove(r),
        }
    }

    // --- Pattern-defining clauses ---

    fn analyze_match(&mut self, m: &MatchClause) -> Result<(), SemanticError> {
        self.analyze_pattern_define(&m.pattern)?;
        if let Some(ref where_expr) = m.where_clause {
            self.analyze_expression_refs(where_expr)?;
        }
        Ok(())
    }

    fn analyze_create(&mut self, c: &CreateClause) -> Result<(), SemanticError> {
        self.analyze_pattern_define(&c.pattern)
    }

    fn analyze_merge(&mut self, m: &MergeClause) -> Result<(), SemanticError> {
        self.analyze_pattern_define(&m.pattern)
    }

    // --- Expression-referencing clauses ---

    fn analyze_return(&mut self, r: &ReturnClause) -> Result<(), SemanticError> {
        for item in &r.items {
            self.analyze_expression_refs(&item.expr)?;
        }
        if let Some(ref order_items) = r.order_by {
            for oi in order_items {
                self.analyze_expression_refs(&oi.expr)?;
            }
        }
        if let Some(ref skip) = r.skip {
            self.analyze_expression_refs(skip)?;
        }
        if let Some(ref limit) = r.limit {
            self.analyze_expression_refs(limit)?;
        }
        Ok(())
    }

    fn analyze_with(&mut self, w: &WithClause) -> Result<(), SemanticError> {
        for item in &w.items {
            self.analyze_expression_refs(&item.expr)?;
        }
        if let Some(ref where_expr) = w.where_clause {
            self.analyze_expression_refs(where_expr)?;
        }
        Ok(())
    }

    fn analyze_set(&mut self, s: &crate::parser::ast::SetClause) -> Result<(), SemanticError> {
        for item in &s.items {
            match item {
                SetItem::Property { target, value } => {
                    self.analyze_expression_refs(target)?;
                    self.analyze_expression_refs(value)?;
                }
            }
        }
        Ok(())
    }

    fn analyze_delete(&mut self, d: &DeleteClause) -> Result<(), SemanticError> {
        for expr in &d.exprs {
            self.analyze_expression_refs(expr)?;
        }
        Ok(())
    }

    fn analyze_remove(
        &mut self,
        r: &crate::parser::ast::RemoveClause,
    ) -> Result<(), SemanticError> {
        for item in &r.items {
            match item {
                RemoveItem::Property(expr) => {
                    self.analyze_expression_refs(expr)?;
                }
                RemoveItem::Label { variable, label } => {
                    if !self.symbols.is_defined(variable) {
                        return Err(SemanticError {
                            message: format!("undefined variable '{}'", variable),
                        });
                    }
                    self.registry.get_or_create_label(label);
                }
            }
        }
        Ok(())
    }

    // --- Pattern definition (defines variables and resolves labels/types) ---

    fn analyze_pattern_define(&mut self, pattern: &Pattern) -> Result<(), SemanticError> {
        for chain in &pattern.chains {
            for element in &chain.elements {
                match element {
                    PatternElement::Node(node) => self.analyze_node_pattern(node)?,
                    PatternElement::Relationship(rel) => self.analyze_rel_pattern(rel)?,
                }
            }
        }
        Ok(())
    }

    fn analyze_node_pattern(&mut self, node: &NodePattern) -> Result<(), SemanticError> {
        // Define variable if present.
        if let Some(ref var) = node.variable {
            self.symbols
                .define(var.clone(), VariableKind::Node)
                .map_err(|msg| SemanticError { message: msg })?;
        }
        // Resolve labels.
        for label in &node.labels {
            self.registry.get_or_create_label(label);
        }
        // Resolve property keys in map literal.
        if let Some(ref props) = node.properties {
            for (key, value) in props {
                self.registry.get_or_create_prop_key(key);
                self.analyze_expression_refs(value)?;
            }
        }
        Ok(())
    }

    fn analyze_rel_pattern(&mut self, rel: &RelationshipPattern) -> Result<(), SemanticError> {
        // Define variable if present.
        if let Some(ref var) = rel.variable {
            self.symbols
                .define(var.clone(), VariableKind::Relationship)
                .map_err(|msg| SemanticError { message: msg })?;
        }
        // Resolve relationship types.
        for rt in &rel.rel_types {
            self.registry.get_or_create_rel_type(rt);
        }
        // Resolve property keys in map literal.
        if let Some(ref props) = rel.properties {
            for (key, value) in props {
                self.registry.get_or_create_prop_key(key);
                self.analyze_expression_refs(value)?;
            }
        }
        Ok(())
    }

    // --- Expression reference checking ---

    fn analyze_expression_refs(&self, expr: &Expression) -> Result<(), SemanticError> {
        match expr {
            Expression::Variable(name) => {
                if !self.symbols.is_defined(name) {
                    return Err(SemanticError {
                        message: format!("undefined variable '{}'", name),
                    });
                }
                Ok(())
            }
            Expression::Property(inner, _prop_key) => {
                // We check the inner expression for variable references.
                // Property key resolution is deferred to planner/executor.
                self.analyze_expression_refs(inner)
            }
            Expression::BinaryOp(_, lhs, rhs) => {
                self.analyze_expression_refs(lhs)?;
                self.analyze_expression_refs(rhs)
            }
            Expression::UnaryOp(_, operand) => self.analyze_expression_refs(operand),
            Expression::FunctionCall { args, .. } => {
                for arg in args {
                    self.analyze_expression_refs(arg)?;
                }
                Ok(())
            }
            Expression::IsNull(inner, _) => self.analyze_expression_refs(inner),
            Expression::Literal(_) | Expression::Parameter(_) | Expression::CountStar => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::*;
    use std::collections::HashMap;

    // --- MockCatalog implementing LabelRegistry ---

    #[derive(Default)]
    struct MockCatalog {
        labels: HashMap<String, u32>,
        rel_types: HashMap<String, u32>,
        prop_keys: HashMap<String, u32>,
        next_label: u32,
        next_rel: u32,
        next_prop: u32,
    }

    impl LabelRegistry for MockCatalog {
        fn get_or_create_label(&mut self, name: &str) -> u32 {
            if let Some(&id) = self.labels.get(name) {
                return id;
            }
            let id = self.next_label;
            self.next_label += 1;
            self.labels.insert(name.to_string(), id);
            id
        }
        fn label_id(&self, name: &str) -> Option<u32> {
            self.labels.get(name).copied()
        }
        fn label_name(&self, _id: u32) -> Option<&str> {
            None // Not needed for tests.
        }
        fn get_or_create_rel_type(&mut self, name: &str) -> u32 {
            if let Some(&id) = self.rel_types.get(name) {
                return id;
            }
            let id = self.next_rel;
            self.next_rel += 1;
            self.rel_types.insert(name.to_string(), id);
            id
        }
        fn rel_type_id(&self, name: &str) -> Option<u32> {
            self.rel_types.get(name).copied()
        }
        fn rel_type_name(&self, _id: u32) -> Option<&str> {
            None
        }
        fn get_or_create_prop_key(&mut self, name: &str) -> u32 {
            if let Some(&id) = self.prop_keys.get(name) {
                return id;
            }
            let id = self.next_prop;
            self.next_prop += 1;
            self.prop_keys.insert(name.to_string(), id);
            id
        }
        fn prop_key_id(&self, name: &str) -> Option<u32> {
            self.prop_keys.get(name).copied()
        }
        fn prop_key_name(&self, _id: u32) -> Option<&str> {
            None
        }
    }

    // --- Helper: build a simple node pattern ---

    fn node(var: Option<&str>, labels: &[&str], props: Option<MapLiteral>) -> PatternElement {
        PatternElement::Node(NodePattern {
            variable: var.map(|s| s.to_string()),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            properties: props,
        })
    }

    fn rel(
        var: Option<&str>,
        types: &[&str],
        dir: RelDirection,
        props: Option<MapLiteral>,
    ) -> PatternElement {
        PatternElement::Relationship(RelationshipPattern {
            variable: var.map(|s| s.to_string()),
            rel_types: types.iter().map(|s| s.to_string()).collect(),
            direction: dir,
            properties: props,
        })
    }

    fn pattern(chains: Vec<Vec<PatternElement>>) -> Pattern {
        Pattern {
            chains: chains
                .into_iter()
                .map(|elements| PatternChain { elements })
                .collect(),
        }
    }

    fn var_expr(name: &str) -> Expression {
        Expression::Variable(name.to_string())
    }

    fn prop_expr(var_name: &str, prop: &str) -> Expression {
        Expression::Property(Box::new(var_expr(var_name)), prop.to_string())
    }

    fn return_clause(items: Vec<ReturnItem>) -> ReturnClause {
        ReturnClause {
            distinct: false,
            items,
            order_by: None,
            skip: None,
            limit: None,
        }
    }

    fn return_item(expr: Expression) -> ReturnItem {
        ReturnItem { expr, alias: None }
    }

    // === TASK-039 Tests ===

    // Valid: MATCH (n:Person) RETURN n.name -- n is defined, name resolves
    #[test]
    fn test_valid_match_return_property() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                    where_clause: None,
                }),
                Clause::Return(return_clause(vec![return_item(prop_expr("n", "name"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());

        let symbols = result.unwrap();
        assert!(symbols.is_defined("n"));
        assert_eq!(symbols.get("n").unwrap().kind, VariableKind::Node);

        // Verify label was resolved in catalog.
        assert!(catalog.label_id("Person").is_some());
    }

    // Valid: MATCH (a)-[r:KNOWS]->(b) RETURN b.name -- a, r, b all defined
    #[test]
    fn test_valid_match_relationship_pattern() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![
                        node(Some("a"), &[], None),
                        rel(Some("r"), &["KNOWS"], RelDirection::Outgoing, None),
                        node(Some("b"), &[], None),
                    ]]),
                    where_clause: None,
                }),
                Clause::Return(return_clause(vec![return_item(prop_expr("b", "name"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());

        let symbols = result.unwrap();
        assert!(symbols.is_defined("a"));
        assert!(symbols.is_defined("r"));
        assert!(symbols.is_defined("b"));
        assert_eq!(symbols.get("r").unwrap().kind, VariableKind::Relationship);

        // Verify relationship type was resolved.
        assert!(catalog.rel_type_id("KNOWS").is_some());
    }

    // Invalid: MATCH (n:Person) RETURN m.name -- m is undefined -> SemanticError
    #[test]
    fn test_invalid_undefined_variable_in_return() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                    where_clause: None,
                }),
                Clause::Return(return_clause(vec![return_item(prop_expr("m", "name"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            err.message.contains("undefined variable 'm'"),
            "expected undefined variable error, got: {}",
            err.message
        );
    }

    // Invalid: RETURN n.name without MATCH -- n undefined
    #[test]
    fn test_invalid_return_without_match() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![Clause::Return(return_clause(vec![return_item(prop_expr(
                "n", "name",
            ))]))],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("undefined variable 'n'"));
    }

    // Valid: CREATE (n:Person {name: "Alice"}) RETURN n -- n defined in CREATE
    #[test]
    fn test_valid_create_with_properties_and_return() {
        let mut catalog = MockCatalog::default();
        let props = vec![(
            "name".to_string(),
            Expression::Literal(Literal::String("Alice".to_string())),
        )];

        let query = Query {
            clauses: vec![
                Clause::Create(CreateClause {
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], Some(props))]]),
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());

        let symbols = result.unwrap();
        assert!(symbols.is_defined("n"));

        // Verify label and prop key were resolved.
        assert!(catalog.label_id("Person").is_some());
        assert!(catalog.prop_key_id("name").is_some());
    }

    // Valid: MATCH (n) WHERE n.age > 30 RETURN n -- WHERE refs n
    #[test]
    fn test_valid_where_references_defined_variable() {
        let mut catalog = MockCatalog::default();
        let where_expr = Expression::BinaryOp(
            BinaryOp::Gt,
            Box::new(prop_expr("n", "age")),
            Box::new(Expression::Literal(Literal::Integer(30))),
        );

        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &[], None)]]),
                    where_clause: Some(where_expr),
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());
    }

    // Invalid: MATCH (n) WHERE m.age > 30 RETURN n -- m undefined in WHERE
    #[test]
    fn test_invalid_undefined_variable_in_where() {
        let mut catalog = MockCatalog::default();
        let where_expr = Expression::BinaryOp(
            BinaryOp::Gt,
            Box::new(prop_expr("m", "age")),
            Box::new(Expression::Literal(Literal::Integer(30))),
        );

        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &[], None)]]),
                    where_clause: Some(where_expr),
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            err.message.contains("undefined variable 'm'"),
            "expected undefined variable error, got: {}",
            err.message
        );
    }

    // Additional: anonymous node patterns (no variable) are allowed
    #[test]
    fn test_valid_anonymous_node_pattern() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![Clause::Match(MatchClause {
                optional: false,
                pattern: pattern(vec![vec![node(None, &["Person"], None)]]),
                where_clause: None,
            })],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());
        assert!(catalog.label_id("Person").is_some());
    }

    // Additional: redefining a node variable with the same kind across patterns is ok
    #[test]
    fn test_valid_redefine_same_kind() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                    where_clause: None,
                }),
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &["Company"], None)]]),
                    where_clause: None,
                }),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());
    }

    // Additional: defining a node variable then redefining as relationship is an error
    #[test]
    fn test_invalid_redefine_different_kind() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &[], None)]]),
                    where_clause: None,
                }),
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![rel(
                        Some("n"),
                        &["KNOWS"],
                        RelDirection::Outgoing,
                        None,
                    )]]),
                    where_clause: None,
                }),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("already defined as"));
    }

    // Additional: SET clause checks variable references
    #[test]
    fn test_valid_set_clause() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &[], None)]]),
                    where_clause: None,
                }),
                Clause::Set(SetClause {
                    items: vec![SetItem::Property {
                        target: prop_expr("n", "age"),
                        value: Expression::Literal(Literal::Integer(42)),
                    }],
                }),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        assert!(analyzer.analyze(&query).is_ok());
    }

    // Additional: DELETE clause checks variable references
    #[test]
    fn test_valid_delete_clause() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &[], None)]]),
                    where_clause: None,
                }),
                Clause::Delete(DeleteClause {
                    detach: true,
                    exprs: vec![var_expr("n")],
                }),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        assert!(analyzer.analyze(&query).is_ok());
    }

    // Additional: DELETE with undefined variable fails
    #[test]
    fn test_invalid_delete_undefined_variable() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![Clause::Delete(DeleteClause {
                detach: false,
                exprs: vec![var_expr("n")],
            })],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("undefined variable 'n'"));
    }

    // Additional: MERGE defines variables like CREATE
    #[test]
    fn test_valid_merge_defines_variables() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Merge(MergeClause {
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        assert!(analyzer.analyze(&query).is_ok());
    }

    // Additional: function calls with variable arguments are checked
    #[test]
    fn test_valid_function_call_in_return() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &[], None)]]),
                    where_clause: None,
                }),
                Clause::Return(return_clause(vec![return_item(Expression::FunctionCall {
                    name: "count".to_string(),
                    distinct: false,
                    args: vec![var_expr("n")],
                })])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        assert!(analyzer.analyze(&query).is_ok());
    }

    // Additional: SemanticError Display implementation
    #[test]
    fn test_semantic_error_display() {
        let err = SemanticError {
            message: "test error".to_string(),
        };
        assert_eq!(format!("{}", err), "Semantic error: test error");
    }
}
