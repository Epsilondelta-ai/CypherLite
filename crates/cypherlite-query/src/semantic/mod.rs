// Semantic analysis: variable scope validation, label/type resolution

use crate::parser::ast::{
    Clause, CreateClause, DeleteClause, Expression, MatchClause, MergeClause, NodePattern, Pattern,
    PatternElement, RelationshipPattern, RemoveItem, ReturnClause, SetItem, UnwindClause,
    WithClause,
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
            Clause::Unwind(u) => self.analyze_unwind(u),
            // DDL clauses: no variable scope validation needed
            Clause::CreateIndex(_) | Clause::DropIndex(_) => Ok(()),
            #[cfg(feature = "subgraph")]
            Clause::CreateSnapshot(_) => Ok(()), // TODO: semantic analysis for snapshot
            #[cfg(feature = "hypergraph")]
            Clause::CreateHyperedge(hc) => {
                // Register the hyperedge variable if present
                if let Some(ref var) = hc.variable {
                    self.symbols
                        .define(var.clone(), VariableKind::Expression)
                        .map_err(|msg| SemanticError { message: msg })?;
                }
                Ok(())
            }
            #[cfg(feature = "hypergraph")]
            Clause::MatchHyperedge(mhc) => {
                // Register the hyperedge variable in scope
                if let Some(ref var) = mhc.variable {
                    self.symbols
                        .define(var.clone(), VariableKind::Expression)
                        .map_err(|msg| SemanticError { message: msg })?;
                }
                Ok(())
            }
        }
    }

    // --- Pattern-defining clauses ---

    fn analyze_match(&mut self, m: &MatchClause) -> Result<(), SemanticError> {
        self.analyze_pattern_define_with_nullable(&m.pattern, m.optional)?;
        // Validate temporal predicate expressions if present
        if let Some(ref tp) = m.temporal_predicate {
            match tp {
                crate::parser::ast::TemporalPredicate::AsOf(expr) => {
                    self.analyze_expression_refs(expr)?;
                }
                crate::parser::ast::TemporalPredicate::Between(start, end) => {
                    self.analyze_expression_refs(start)?;
                    self.analyze_expression_refs(end)?;
                }
            }
        }
        if let Some(ref where_expr) = m.where_clause {
            self.analyze_expression_refs(where_expr)?;
        }
        Ok(())
    }

    fn analyze_create(&mut self, c: &CreateClause) -> Result<(), SemanticError> {
        self.analyze_pattern_define(&c.pattern)
    }

    fn analyze_merge(&mut self, m: &MergeClause) -> Result<(), SemanticError> {
        self.analyze_pattern_define(&m.pattern)?;
        // Validate ON MATCH SET items reference defined variables
        for item in &m.on_match {
            match item {
                SetItem::Property { target, value } => {
                    self.analyze_expression_refs(target)?;
                    self.analyze_expression_refs(value)?;
                }
            }
        }
        // Validate ON CREATE SET items reference defined variables
        for item in &m.on_create {
            match item {
                SetItem::Property { target, value } => {
                    self.analyze_expression_refs(target)?;
                    self.analyze_expression_refs(value)?;
                }
            }
        }
        Ok(())
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
        // First, validate all WITH expressions against current scope
        for item in &w.items {
            self.analyze_expression_refs(&item.expr)?;
        }

        // Determine surviving variables after scope reset.
        // Each WITH item produces a variable: alias if present, or variable name from expression.
        let survivors: Vec<(String, VariableKind)> = w
            .items
            .iter()
            .filter_map(|item| {
                let name = match &item.alias {
                    Some(alias) => alias.clone(),
                    None => match &item.expr {
                        Expression::Variable(v) => v.clone(),
                        _ => return None,
                    },
                };
                // If the expression is a plain variable and it already has a kind, preserve it.
                // Otherwise, it becomes an Expression kind.
                let kind = if item.alias.is_none() {
                    if let Expression::Variable(v) = &item.expr {
                        self.symbols
                            .get(v)
                            .map(|info| info.kind)
                            .unwrap_or(VariableKind::Expression)
                    } else {
                        VariableKind::Expression
                    }
                } else {
                    VariableKind::Expression
                };
                Some((name, kind))
            })
            .collect();

        // Reset scope: only projected variables survive
        self.symbols.reset_scope(&survivors);

        // Validate WITH WHERE against the new scope (after reset)
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

    fn analyze_unwind(&mut self, u: &UnwindClause) -> Result<(), SemanticError> {
        self.analyze_expression_refs(&u.expr)?;
        self.symbols
            .define(u.variable.clone(), VariableKind::Expression)
            .map_err(|msg| SemanticError { message: msg })?;
        Ok(())
    }

    // --- Pattern definition (defines variables and resolves labels/types) ---

    fn analyze_pattern_define(&mut self, pattern: &Pattern) -> Result<(), SemanticError> {
        self.analyze_pattern_define_with_nullable(pattern, false)
    }

    fn analyze_pattern_define_with_nullable(
        &mut self,
        pattern: &Pattern,
        nullable: bool,
    ) -> Result<(), SemanticError> {
        for chain in &pattern.chains {
            for element in &chain.elements {
                match element {
                    PatternElement::Node(node) => {
                        self.analyze_node_pattern(node, nullable)?;
                    }
                    PatternElement::Relationship(rel) => {
                        self.analyze_rel_pattern(rel, nullable)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn analyze_node_pattern(
        &mut self,
        node: &NodePattern,
        nullable: bool,
    ) -> Result<(), SemanticError> {
        // Define variable if present.
        if let Some(ref var) = node.variable {
            self.symbols
                .define_with_nullable(var.clone(), VariableKind::Node, nullable)
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

    fn analyze_rel_pattern(
        &mut self,
        rel: &RelationshipPattern,
        nullable: bool,
    ) -> Result<(), SemanticError> {
        // Define variable if present.
        if let Some(ref var) = rel.variable {
            self.symbols
                .define_with_nullable(var.clone(), VariableKind::Relationship, nullable)
                .map_err(|msg| SemanticError { message: msg })?;
        }
        // Resolve relationship types.
        for rt in &rel.rel_types {
            self.registry.get_or_create_rel_type(rt);
        }
        // Validate variable-length path bounds.
        if let (Some(min), Some(max)) = (rel.min_hops, rel.max_hops) {
            if max < min {
                return Err(SemanticError {
                    message: format!("max_hops ({}) must be >= min_hops ({})", max, min),
                });
            }
        }
        // Configurable max hop limit (default 10).
        const MAX_HOP_LIMIT: u32 = 10;
        if let Some(max) = rel.max_hops {
            if max > MAX_HOP_LIMIT {
                return Err(SemanticError {
                    message: format!(
                        "max_hops ({}) exceeds configurable limit ({})",
                        max, MAX_HOP_LIMIT
                    ),
                });
            }
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
            Expression::ListLiteral(elements) => {
                for elem in elements {
                    self.analyze_expression_refs(elem)?;
                }
                Ok(())
            }
            Expression::Literal(_) | Expression::Parameter(_) | Expression::CountStar => Ok(()),
            #[cfg(feature = "hypergraph")]
            Expression::TemporalRef { node, timestamp } => {
                self.analyze_expression_refs(node)?;
                self.analyze_expression_refs(timestamp)
            }
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
            min_hops: None,
            max_hops: None,
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
                    temporal_predicate: None,
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
                    temporal_predicate: None,
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
                    temporal_predicate: None,
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
                    temporal_predicate: None,
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
                    temporal_predicate: None,
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
                temporal_predicate: None,
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
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &["Company"], None)]]),
                    temporal_predicate: None,
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
                    temporal_predicate: None,
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
                    temporal_predicate: None,
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
                    temporal_predicate: None,
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
                    temporal_predicate: None,
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
                    on_match: vec![],
                    on_create: vec![],
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        assert!(analyzer.analyze(&query).is_ok());
    }

    // TASK-085: MERGE with ON MATCH SET / ON CREATE SET validates variable refs
    #[test]
    fn test_valid_merge_on_match_set() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Merge(MergeClause {
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                    on_match: vec![SetItem::Property {
                        target: prop_expr("n", "seen"),
                        value: Expression::Literal(Literal::Bool(true)),
                    }],
                    on_create: vec![],
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        assert!(analyzer.analyze(&query).is_ok());
    }

    // TASK-085: MERGE ON CREATE SET with undefined variable fails
    #[test]
    fn test_invalid_merge_on_create_set_undefined_var() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![Clause::Merge(MergeClause {
                pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                on_match: vec![],
                on_create: vec![SetItem::Property {
                    target: prop_expr("m", "created"),
                    value: Expression::Literal(Literal::Bool(true)),
                }],
            })],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("undefined variable 'm'"));
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
                    temporal_predicate: None,
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

    // === TASK-060 Tests: WITH clause scope reset ===

    fn with_clause(items: Vec<ReturnItem>, where_clause: Option<Expression>) -> WithClause {
        WithClause {
            distinct: false,
            items,
            where_clause,
        }
    }

    // MATCH (n:Person)-[r:KNOWS]->(m:Person) WITH n RETURN n
    // After WITH n, only 'n' survives; 'm' and 'r' become inaccessible
    #[test]
    fn test_with_scope_reset_projected_variable_survives() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![
                        node(Some("n"), &["Person"], None),
                        rel(Some("r"), &["KNOWS"], RelDirection::Outgoing, None),
                        node(Some("m"), &["Person"], None),
                    ]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::With(with_clause(vec![return_item(var_expr("n"))], None)),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());
    }

    // MATCH (n:Person)-[r:KNOWS]->(m:Person) WITH n RETURN m
    // Error: 'm' not in WITH projection, so it is undefined after WITH
    #[test]
    fn test_with_scope_reset_non_projected_variable_error() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![
                        node(Some("n"), &["Person"], None),
                        rel(Some("r"), &["KNOWS"], RelDirection::Outgoing, None),
                        node(Some("m"), &["Person"], None),
                    ]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::With(with_clause(vec![return_item(var_expr("n"))], None)),
                Clause::Return(return_clause(vec![return_item(var_expr("m"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("undefined variable 'm'"));
    }

    // MATCH (n:Person) WITH n.name AS name RETURN name
    // The alias 'name' is available after WITH, but 'n' is not
    #[test]
    fn test_with_alias_creates_new_scope() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::With(with_clause(
                    vec![ReturnItem {
                        expr: prop_expr("n", "name"),
                        alias: Some("name".to_string()),
                    }],
                    None,
                )),
                Clause::Return(return_clause(vec![return_item(var_expr("name"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());
    }

    // MATCH (n:Person) WITH n.name AS name RETURN n
    // Error: 'n' is not in WITH projection, only 'name' alias is
    #[test]
    fn test_with_alias_original_variable_inaccessible() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::With(with_clause(
                    vec![ReturnItem {
                        expr: prop_expr("n", "name"),
                        alias: Some("name".to_string()),
                    }],
                    None,
                )),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("undefined variable 'n'"));
    }

    // MATCH (n:Person) WITH n WHERE n.age > 30 RETURN n
    // WITH WHERE should be able to reference projected variables
    #[test]
    fn test_with_where_references_projected_variable() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &["Person"], None)]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::With(with_clause(
                    vec![return_item(var_expr("n"))],
                    Some(Expression::BinaryOp(
                        BinaryOp::Gt,
                        Box::new(prop_expr("n", "age")),
                        Box::new(Expression::Literal(Literal::Integer(30))),
                    )),
                )),
                Clause::Return(return_clause(vec![return_item(var_expr("n"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());
    }

    // === TASK-074 Tests: OPTIONAL MATCH semantic analysis ===

    // OPTIONAL MATCH variables should be marked as nullable
    #[test]
    fn test_optional_match_variables_are_nullable() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("a"), &["Person"], None)]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::Match(MatchClause {
                    optional: true,
                    pattern: pattern(vec![vec![
                        node(Some("a"), &[], None),
                        rel(Some("r"), &["KNOWS"], RelDirection::Outgoing, None),
                        node(Some("b"), &[], None),
                    ]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::Return(return_clause(vec![
                    return_item(prop_expr("a", "name")),
                    return_item(prop_expr("b", "name")),
                ])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());

        let symbols = result.unwrap();
        // 'a' from regular MATCH is not nullable
        assert!(!symbols.get("a").unwrap().nullable);
        // 'b' from OPTIONAL MATCH is nullable
        assert!(symbols.get("b").unwrap().nullable);
        // 'r' from OPTIONAL MATCH is nullable
        assert!(symbols.get("r").unwrap().nullable);
    }

    // OPTIONAL MATCH can reference variables from earlier MATCH
    #[test]
    fn test_optional_match_references_earlier_match_variable() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("a"), &["Person"], None)]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::Match(MatchClause {
                    optional: true,
                    pattern: pattern(vec![vec![
                        node(Some("a"), &[], None),
                        rel(None, &["WORKS_AT"], RelDirection::Outgoing, None),
                        node(Some("c"), &["Company"], None),
                    ]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::Return(return_clause(vec![
                    return_item(var_expr("a")),
                    return_item(var_expr("c")),
                ])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());

        let symbols = result.unwrap();
        // 'a' was first defined in regular MATCH (not nullable), then re-referenced in OPTIONAL MATCH.
        // The non-nullable definition should be preserved.
        assert!(!symbols.get("a").unwrap().nullable);
        // 'c' is from OPTIONAL MATCH, so nullable
        assert!(symbols.get("c").unwrap().nullable);
    }

    // OPTIONAL MATCH with WHERE clause
    #[test]
    fn test_optional_match_with_where() {
        let mut catalog = MockCatalog::default();
        let where_expr = Expression::BinaryOp(
            BinaryOp::Gt,
            Box::new(prop_expr("b", "age")),
            Box::new(Expression::Literal(Literal::Integer(20))),
        );

        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("a"), &["Person"], None)]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::Match(MatchClause {
                    optional: true,
                    pattern: pattern(vec![vec![
                        node(Some("a"), &[], None),
                        rel(None, &["KNOWS"], RelDirection::Outgoing, None),
                        node(Some("b"), &[], None),
                    ]]),
                    temporal_predicate: None,
                    where_clause: Some(where_expr),
                }),
                Clause::Return(return_clause(vec![
                    return_item(var_expr("a")),
                    return_item(var_expr("b")),
                ])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());
    }

    // === TASK-069 Tests: UNWIND clause semantic analysis ===

    // UNWIND [1,2,3] AS x RETURN x -- x should be defined after UNWIND
    #[test]
    fn test_unwind_defines_variable() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Unwind(UnwindClause {
                    expr: Expression::ListLiteral(vec![
                        Expression::Literal(Literal::Integer(1)),
                        Expression::Literal(Literal::Integer(2)),
                    ]),
                    variable: "x".to_string(),
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("x"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());

        let symbols = result.unwrap();
        assert!(symbols.is_defined("x"));
        assert_eq!(symbols.get("x").unwrap().kind, VariableKind::Expression);
    }

    // MATCH (n) UNWIND n.hobbies AS h RETURN h -- UNWIND expr references n
    #[test]
    fn test_unwind_references_prior_variables() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Match(MatchClause {
                    optional: false,
                    pattern: pattern(vec![vec![node(Some("n"), &[], None)]]),
                    temporal_predicate: None,
                    where_clause: None,
                }),
                Clause::Unwind(UnwindClause {
                    expr: prop_expr("n", "hobbies"),
                    variable: "h".to_string(),
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("h"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_ok());
    }

    // UNWIND m.items AS x RETURN x -- m is undefined -> error
    #[test]
    fn test_unwind_undefined_variable_in_expr() {
        let mut catalog = MockCatalog::default();
        let query = Query {
            clauses: vec![
                Clause::Unwind(UnwindClause {
                    expr: prop_expr("m", "items"),
                    variable: "x".to_string(),
                }),
                Clause::Return(return_clause(vec![return_item(var_expr("x"))])),
            ],
        };

        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let result = analyzer.analyze(&query);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("undefined variable 'm'"));
    }

    // Additional: SemanticError Display implementation
    #[test]
    fn test_semantic_error_display() {
        let err = SemanticError {
            message: "test error".to_string(),
        };
        assert_eq!(format!("{}", err), "Semantic error: test error");
    }

    // -- TASK-103: Variable-length path semantic validation --

    #[test]
    fn test_var_length_path_valid() {
        let mut catalog = MockCatalog::default();
        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let query = Query {
            clauses: vec![Clause::Match(MatchClause {
                optional: false,
                pattern: pattern(vec![vec![
                    node(Some("a"), &["Person"], None),
                    rel(None, &["KNOWS"], RelDirection::Outgoing, None),
                    node(Some("b"), &[], None),
                ]]),
                temporal_predicate: None,
                where_clause: None,
            })],
        };
        // Modify the relationship to have variable-length
        let mut q = query;
        if let Clause::Match(ref mut mc) = q.clauses[0] {
            if let PatternElement::Relationship(ref mut rp) = mc.pattern.chains[0].elements[1] {
                rp.min_hops = Some(1);
                rp.max_hops = Some(3);
            }
        }
        assert!(analyzer.analyze(&q).is_ok());
    }

    #[test]
    fn test_var_length_path_max_less_than_min() {
        let mut catalog = MockCatalog::default();
        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let mut query = Query {
            clauses: vec![Clause::Match(MatchClause {
                optional: false,
                pattern: pattern(vec![vec![
                    node(Some("a"), &[], None),
                    rel(None, &[], RelDirection::Outgoing, None),
                    node(Some("b"), &[], None),
                ]]),
                temporal_predicate: None,
                where_clause: None,
            })],
        };
        if let Clause::Match(ref mut mc) = query.clauses[0] {
            if let PatternElement::Relationship(ref mut rp) = mc.pattern.chains[0].elements[1] {
                rp.min_hops = Some(5);
                rp.max_hops = Some(2);
            }
        }
        let result = analyzer.analyze(&query);
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .message
            .contains("max_hops"));
    }

    #[test]
    fn test_var_length_path_max_exceeds_limit() {
        let mut catalog = MockCatalog::default();
        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let mut query = Query {
            clauses: vec![Clause::Match(MatchClause {
                optional: false,
                pattern: pattern(vec![vec![
                    node(Some("a"), &[], None),
                    rel(None, &[], RelDirection::Outgoing, None),
                    node(Some("b"), &[], None),
                ]]),
                temporal_predicate: None,
                where_clause: None,
            })],
        };
        if let Clause::Match(ref mut mc) = query.clauses[0] {
            if let PatternElement::Relationship(ref mut rp) = mc.pattern.chains[0].elements[1] {
                rp.min_hops = Some(1);
                rp.max_hops = Some(100);
            }
        }
        let result = analyzer.analyze(&query);
        assert!(result.is_err());
        assert!(result.expect_err("should fail").message.contains("exceeds"));
    }

    #[test]
    fn test_var_length_path_unbounded_ok() {
        let mut catalog = MockCatalog::default();
        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let mut query = Query {
            clauses: vec![Clause::Match(MatchClause {
                optional: false,
                pattern: pattern(vec![vec![
                    node(Some("a"), &[], None),
                    rel(None, &[], RelDirection::Outgoing, None),
                    node(Some("b"), &[], None),
                ]]),
                temporal_predicate: None,
                where_clause: None,
            })],
        };
        if let Clause::Match(ref mut mc) = query.clauses[0] {
            if let PatternElement::Relationship(ref mut rp) = mc.pattern.chains[0].elements[1] {
                rp.min_hops = Some(1);
                rp.max_hops = None; // unbounded is OK - planner will cap it
            }
        }
        assert!(analyzer.analyze(&query).is_ok());
    }

    #[test]
    fn test_var_length_path_min_zero_ok() {
        let mut catalog = MockCatalog::default();
        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let mut query = Query {
            clauses: vec![Clause::Match(MatchClause {
                optional: false,
                pattern: pattern(vec![vec![
                    node(Some("a"), &[], None),
                    rel(None, &[], RelDirection::Outgoing, None),
                    node(Some("b"), &[], None),
                ]]),
                temporal_predicate: None,
                where_clause: None,
            })],
        };
        if let Clause::Match(ref mut mc) = query.clauses[0] {
            if let PatternElement::Relationship(ref mut rp) = mc.pattern.chains[0].elements[1] {
                rp.min_hops = Some(0);
                rp.max_hops = Some(1);
            }
        }
        assert!(analyzer.analyze(&query).is_ok());
    }

    #[test]
    fn test_var_length_path_equal_min_max_ok() {
        let mut catalog = MockCatalog::default();
        let mut analyzer = SemanticAnalyzer::new(&mut catalog);
        let mut query = Query {
            clauses: vec![Clause::Match(MatchClause {
                optional: false,
                pattern: pattern(vec![vec![
                    node(Some("a"), &[], None),
                    rel(None, &[], RelDirection::Outgoing, None),
                    node(Some("b"), &[], None),
                ]]),
                temporal_predicate: None,
                where_clause: None,
            })],
        };
        if let Clause::Match(ref mut mc) = query.clauses[0] {
            if let PatternElement::Relationship(ref mut rp) = mc.pattern.chains[0].elements[1] {
                rp.min_hops = Some(3);
                rp.max_hops = Some(3);
            }
        }
        assert!(analyzer.analyze(&query).is_ok());
    }
}
