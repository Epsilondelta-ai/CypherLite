// Clause parsing: MATCH, RETURN, CREATE, SET, DELETE, REMOVE, WITH, MERGE

use super::ast::*;
use super::{ParseError, Parser};
use crate::lexer::Token;

impl<'a> Parser<'a> {
    /// Parse a MATCH clause (TASK-028).
    ///
    /// Expects the parser to be positioned at the MATCH keyword.
    /// The `optional` flag indicates whether OPTIONAL was already consumed
    /// by the caller.
    ///
    /// Grammar: MATCH pattern [WHERE expression]
    pub fn parse_match_clause(&mut self, optional: bool) -> Result<MatchClause, ParseError> {
        self.expect(&Token::Match)?;
        let pattern = self.parse_pattern()?;

        // Parse optional temporal predicate: AT TIME <expr> | BETWEEN TIME <expr> AND <expr>
        let temporal_predicate = self.parse_optional_temporal_predicate()?;

        let where_clause = if self.eat(&Token::Where) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(MatchClause {
            optional,
            pattern,
            temporal_predicate,
            where_clause,
        })
    }

    /// Parse a RETURN clause (TASK-029, TASK-030).
    ///
    /// Grammar: RETURN [DISTINCT] items [ORDER BY order_items] [SKIP expr] [LIMIT expr]
    pub fn parse_return_clause(&mut self) -> Result<ReturnClause, ParseError> {
        self.expect(&Token::Return)?;

        let distinct = self.eat(&Token::Distinct);

        let items = self.parse_return_items()?;

        let order_by = if self.check(&Token::Order) {
            Some(self.parse_order_by()?)
        } else {
            None
        };

        let skip = if self.eat(&Token::Skip) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        let limit = if self.eat(&Token::Limit) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(ReturnClause {
            distinct,
            items,
            order_by,
            skip,
            limit,
        })
    }

    /// Parse a CREATE clause (TASK-031).
    ///
    /// Grammar: CREATE pattern
    pub fn parse_create_clause(&mut self) -> Result<CreateClause, ParseError> {
        self.expect(&Token::Create)?;
        let pattern = self.parse_pattern()?;
        Ok(CreateClause { pattern })
    }

    /// Parse a SET clause (TASK-032).
    ///
    /// Grammar: SET property = expression [, property = expression]*
    pub fn parse_set_clause(&mut self) -> Result<SetClause, ParseError> {
        self.expect(&Token::Set)?;

        let mut items = Vec::new();
        items.push(self.parse_set_item()?);
        while self.eat(&Token::Comma) {
            items.push(self.parse_set_item()?);
        }

        Ok(SetClause { items })
    }

    /// Parse a REMOVE clause (TASK-032).
    ///
    /// Grammar: REMOVE (property_access | variable:Label) [, ...]*
    pub fn parse_remove_clause(&mut self) -> Result<RemoveClause, ParseError> {
        self.expect(&Token::Remove)?;

        let mut items = Vec::new();
        items.push(self.parse_remove_item()?);
        while self.eat(&Token::Comma) {
            items.push(self.parse_remove_item()?);
        }

        Ok(RemoveClause { items })
    }

    /// Parse a DELETE clause (TASK-033).
    ///
    /// Grammar: DELETE expression [, expression]*
    /// The `detach` flag indicates whether DETACH was already consumed.
    pub fn parse_delete_clause(&mut self, detach: bool) -> Result<DeleteClause, ParseError> {
        self.expect(&Token::Delete)?;

        let mut exprs = Vec::new();
        exprs.push(self.parse_expression()?);
        while self.eat(&Token::Comma) {
            exprs.push(self.parse_expression()?);
        }

        Ok(DeleteClause { detach, exprs })
    }

    /// Parse a WITH clause (P2 syntax -- parsed but execution returns UnsupportedSyntax).
    ///
    /// Grammar: WITH [DISTINCT] items [WHERE expression]
    pub fn parse_with_clause(&mut self) -> Result<WithClause, ParseError> {
        self.expect(&Token::With)?;

        let distinct = self.eat(&Token::Distinct);
        let items = self.parse_return_items()?;

        let where_clause = if self.eat(&Token::Where) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(WithClause {
            distinct,
            items,
            where_clause,
        })
    }

    /// Parse an UNWIND clause (TASK-068).
    ///
    /// Grammar: UNWIND expression AS variable
    pub fn parse_unwind_clause(&mut self) -> Result<UnwindClause, ParseError> {
        self.expect(&Token::Unwind)?;
        let expr = self.parse_expression()?;
        self.expect(&Token::As)?;
        let variable = self.expect_ident()?;
        Ok(UnwindClause { expr, variable })
    }

    /// Parse a MERGE clause.
    ///
    /// Grammar: MERGE pattern [ON MATCH SET items] [ON CREATE SET items]
    pub fn parse_merge_clause(&mut self) -> Result<MergeClause, ParseError> {
        self.expect(&Token::Merge)?;
        let pattern = self.parse_pattern()?;

        let mut on_match = Vec::new();
        let mut on_create = Vec::new();

        // Parse optional ON MATCH SET and ON CREATE SET (can appear in any order)
        loop {
            if self.check(&Token::On) {
                // Peek at the token after ON
                let next = self.tokens.get(self.pos + 1).map(|(t, _)| t.clone());
                match next {
                    Some(Token::Match) => {
                        self.advance(); // consume ON
                        self.advance(); // consume MATCH
                        self.expect(&Token::Set)?;
                        on_match.push(self.parse_set_item()?);
                        while self.eat(&Token::Comma) {
                            on_match.push(self.parse_set_item()?);
                        }
                    }
                    Some(Token::Create) => {
                        self.advance(); // consume ON
                        self.advance(); // consume CREATE
                        self.expect(&Token::Set)?;
                        on_create.push(self.parse_set_item()?);
                        while self.eat(&Token::Comma) {
                            on_create.push(self.parse_set_item()?);
                        }
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }

        Ok(MergeClause {
            pattern,
            on_match,
            on_create,
        })
    }

    /// Parse a CREATE INDEX clause (TASK-098).
    ///
    /// Grammar: CREATE INDEX [name] ON :Label(property)
    /// The parser has already consumed CREATE and is positioned at INDEX.
    pub fn parse_create_index_clause(&mut self) -> Result<CreateIndexClause, ParseError> {
        self.expect(&Token::Index)?;

        // Optional index name (identifier before ON)
        let name = if !self.check(&Token::On) {
            Some(self.expect_ident()?)
        } else {
            None
        };

        self.expect(&Token::On)?;
        self.expect(&Token::Colon)?;
        let label = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let property = self.expect_ident()?;
        self.expect(&Token::RParen)?;

        Ok(CreateIndexClause {
            name,
            target: IndexTarget::NodeLabel(label),
            property,
        })
    }

    /// Parse a CREATE EDGE INDEX clause (CC-T3).
    ///
    /// Grammar: CREATE EDGE INDEX [name] ON :RelType(property)
    /// The parser has already consumed CREATE and is positioned at EDGE.
    pub fn parse_create_edge_index_clause(&mut self) -> Result<CreateIndexClause, ParseError> {
        self.expect(&Token::Edge)?;
        self.expect(&Token::Index)?;

        // Optional index name (identifier before ON)
        let name = if !self.check(&Token::On) {
            Some(self.expect_ident()?)
        } else {
            None
        };

        self.expect(&Token::On)?;
        self.expect(&Token::Colon)?;
        let rel_type = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let property = self.expect_ident()?;
        self.expect(&Token::RParen)?;

        Ok(CreateIndexClause {
            name,
            target: IndexTarget::RelationshipType(rel_type),
            property,
        })
    }

    /// Parse a DROP INDEX clause (TASK-098).
    ///
    /// Grammar: DROP INDEX name
    pub fn parse_drop_index_clause(&mut self) -> Result<DropIndexClause, ParseError> {
        self.expect(&Token::Drop)?;
        self.expect(&Token::Index)?;
        let name = self.expect_ident()?;
        Ok(DropIndexClause { name })
    }

    /// Parse an optional temporal predicate after MATCH pattern.
    ///
    /// Grammar:
    ///   AT TIME <expression>
    ///   BETWEEN TIME <expression> AND <expression>
    fn parse_optional_temporal_predicate(
        &mut self,
    ) -> Result<Option<TemporalPredicate>, ParseError> {
        if self.check(&Token::At) {
            // AT TIME <expr>
            self.advance(); // consume AT
            self.expect(&Token::Time)?;
            let expr = self.parse_expression()?;
            Ok(Some(TemporalPredicate::AsOf(expr)))
        } else if self.check(&Token::Between) {
            // BETWEEN TIME <expr> AND <expr>
            self.advance(); // consume BETWEEN
            self.expect(&Token::Time)?;
            let start = self.parse_expression_no_and()?;
            self.expect(&Token::And)?;
            let end = self.parse_expression_no_and()?;
            Ok(Some(TemporalPredicate::Between(start, end)))
        } else {
            Ok(None)
        }
    }

    // -- Helper functions --

    /// Parse comma-separated return items: expression [AS alias] [, ...]
    fn parse_return_items(&mut self) -> Result<Vec<ReturnItem>, ParseError> {
        let mut items = Vec::new();
        items.push(self.parse_return_item()?);
        while self.eat(&Token::Comma) {
            items.push(self.parse_return_item()?);
        }
        Ok(items)
    }

    /// Parse a single return item: expression [AS alias]
    fn parse_return_item(&mut self) -> Result<ReturnItem, ParseError> {
        let expr = self.parse_expression()?;
        let alias = if self.eat(&Token::As) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        Ok(ReturnItem { expr, alias })
    }

    /// Parse ORDER BY clause: ORDER BY expression [ASC|DESC] [, ...]
    fn parse_order_by(&mut self) -> Result<Vec<OrderItem>, ParseError> {
        self.expect(&Token::Order)?;
        self.expect(&Token::By)?;

        let mut items = Vec::new();
        items.push(self.parse_order_item()?);
        while self.eat(&Token::Comma) {
            items.push(self.parse_order_item()?);
        }
        Ok(items)
    }

    /// Parse a single order item: expression [ASC|DESC]
    fn parse_order_item(&mut self) -> Result<OrderItem, ParseError> {
        let expr = self.parse_expression()?;
        let ascending = if self.eat(&Token::Desc) {
            false
        } else {
            // ASC is default; consume it if present
            self.eat(&Token::Asc);
            true
        };
        Ok(OrderItem { expr, ascending })
    }

    /// Parse a single SET item: variable.property = expression
    ///
    /// We cannot use parse_expression() for the target because it would
    /// consume the `=` as a comparison operator. Instead, we parse the
    /// property access manually: ident (.ident)*
    fn parse_set_item(&mut self) -> Result<SetItem, ParseError> {
        let name = self.expect_ident()?;
        let mut target = Expression::Variable(name);
        // Parse property chain: .prop1.prop2...
        while self.eat(&Token::Dot) {
            let prop = self.expect_ident()?;
            target = Expression::Property(Box::new(target), prop);
        }
        self.expect(&Token::Eq)?;
        let value = self.parse_expression()?;
        Ok(SetItem::Property { target, value })
    }

    /// Parse a single REMOVE item: property_access or variable:Label
    fn parse_remove_item(&mut self) -> Result<RemoveItem, ParseError> {
        let name = self.expect_ident()?;

        if self.eat(&Token::Colon) {
            // variable:Label
            let label = self.expect_ident()?;
            Ok(RemoveItem::Label {
                variable: name,
                label,
            })
        } else if self.eat(&Token::Dot) {
            // variable.property -- build the property access expression
            let prop = self.expect_ident()?;
            let expr = Expression::Property(Box::new(Expression::Variable(name)), prop);
            Ok(RemoveItem::Property(expr))
        } else {
            Err(self.error("expected '.' or ':' after identifier in REMOVE item"))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    /// Helper: create a parser from an input string.
    fn make_parser(input: &str) -> (Vec<(Token, crate::lexer::Span)>, String) {
        let tokens = lex(input).expect("lexing should succeed");
        (tokens, input.to_string())
    }

    // ======================================================================
    // TASK-028: parse_match_clause
    // ======================================================================

    #[test]
    fn match_simple_node() {
        let (tokens, input) = make_parser("MATCH (n:Person)");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert!(!mc.optional);
        assert_eq!(mc.pattern.chains.len(), 1);
        let node = match &mc.pattern.chains[0].elements[0] {
            PatternElement::Node(n) => n,
            _ => panic!("expected node"),
        };
        assert_eq!(node.variable, Some("n".to_string()));
        assert_eq!(node.labels, vec!["Person".to_string()]);
        assert!(mc.where_clause.is_none());
    }

    #[test]
    fn match_with_where() {
        let (tokens, input) = make_parser("MATCH (n:Person) WHERE n.age > 30");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert!(mc.where_clause.is_some());
        let where_expr = mc.where_clause.expect("checked above");
        assert!(matches!(
            where_expr,
            Expression::BinaryOp(BinaryOp::Gt, _, _)
        ));
    }

    #[test]
    fn match_optional() {
        let (tokens, input) = make_parser("MATCH (n)");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(true).expect("should parse");

        assert!(mc.optional);
    }

    #[test]
    fn match_with_relationship() {
        let (tokens, input) = make_parser("MATCH (a)-[:KNOWS]->(b)");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert_eq!(mc.pattern.chains[0].elements.len(), 3);
    }

    // ======================================================================
    // TASK-029 / TASK-030: parse_return_clause (with ORDER BY, SKIP, LIMIT)
    // ======================================================================

    #[test]
    fn return_simple_variable() {
        let (tokens, input) = make_parser("RETURN n");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        assert!(!rc.distinct);
        assert_eq!(rc.items.len(), 1);
        assert_eq!(rc.items[0].expr, Expression::Variable("n".to_string()));
        assert!(rc.items[0].alias.is_none());
        assert!(rc.order_by.is_none());
        assert!(rc.skip.is_none());
        assert!(rc.limit.is_none());
    }

    #[test]
    fn return_distinct() {
        let (tokens, input) = make_parser("RETURN DISTINCT n.name");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        assert!(rc.distinct);
    }

    #[test]
    fn return_multiple_with_alias() {
        let (tokens, input) = make_parser("RETURN n.name AS name, n.age AS age");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        assert_eq!(rc.items.len(), 2);
        assert_eq!(rc.items[0].alias, Some("name".to_string()));
        assert_eq!(rc.items[1].alias, Some("age".to_string()));
    }

    #[test]
    fn return_with_order_by() {
        let (tokens, input) = make_parser("RETURN n.name ORDER BY n.name ASC");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        let order = rc.order_by.expect("should have ORDER BY");
        assert_eq!(order.len(), 1);
        assert!(order[0].ascending);
    }

    #[test]
    fn return_with_order_by_desc() {
        let (tokens, input) = make_parser("RETURN n ORDER BY n.age DESC");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        let order = rc.order_by.expect("should have ORDER BY");
        assert!(!order[0].ascending);
    }

    #[test]
    fn return_with_order_by_multiple() {
        let (tokens, input) = make_parser("RETURN n ORDER BY n.name ASC, n.age DESC");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        let order = rc.order_by.expect("should have ORDER BY");
        assert_eq!(order.len(), 2);
        assert!(order[0].ascending);
        assert!(!order[1].ascending);
    }

    #[test]
    fn return_with_skip() {
        let (tokens, input) = make_parser("RETURN n SKIP 5");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        assert_eq!(
            rc.skip.expect("should have SKIP"),
            Expression::Literal(Literal::Integer(5))
        );
    }

    #[test]
    fn return_with_limit() {
        let (tokens, input) = make_parser("RETURN n LIMIT 10");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        assert_eq!(
            rc.limit.expect("should have LIMIT"),
            Expression::Literal(Literal::Integer(10))
        );
    }

    #[test]
    fn return_with_order_skip_limit() {
        let (tokens, input) = make_parser("RETURN n ORDER BY n.name SKIP 5 LIMIT 10");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        assert!(rc.order_by.is_some());
        assert!(rc.skip.is_some());
        assert!(rc.limit.is_some());
    }

    #[test]
    fn return_order_by_default_ascending() {
        let (tokens, input) = make_parser("RETURN n ORDER BY n.name");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_return_clause().expect("should parse");

        let order = rc.order_by.expect("should have ORDER BY");
        assert!(order[0].ascending); // default is ASC
    }

    // ======================================================================
    // TASK-031: parse_create_clause
    // ======================================================================

    #[test]
    fn create_simple_node() {
        let (tokens, input) = make_parser("CREATE (n:Person)");
        let mut p = Parser::new(&tokens, &input);
        let cc = p.parse_create_clause().expect("should parse");

        let node = match &cc.pattern.chains[0].elements[0] {
            PatternElement::Node(n) => n,
            _ => panic!("expected node"),
        };
        assert_eq!(node.variable, Some("n".to_string()));
        assert_eq!(node.labels, vec!["Person".to_string()]);
    }

    #[test]
    fn create_node_with_properties() {
        let (tokens, input) = make_parser("CREATE (n:Person {name: 'Alice', age: 30})");
        let mut p = Parser::new(&tokens, &input);
        let cc = p.parse_create_clause().expect("should parse");

        let node = match &cc.pattern.chains[0].elements[0] {
            PatternElement::Node(n) => n,
            _ => panic!("expected node"),
        };
        assert!(node.properties.is_some());
        let props = node.properties.as_ref().expect("checked above");
        assert_eq!(props.len(), 2);
    }

    #[test]
    fn create_relationship() {
        let (tokens, input) =
            make_parser("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})");
        let mut p = Parser::new(&tokens, &input);
        let cc = p.parse_create_clause().expect("should parse");

        assert_eq!(cc.pattern.chains[0].elements.len(), 3);
    }

    // ======================================================================
    // TASK-032: parse_set_clause / parse_remove_clause
    // ======================================================================

    #[test]
    fn set_single_property() {
        let (tokens, input) = make_parser("SET n.name = 'Alice'");
        let mut p = Parser::new(&tokens, &input);
        let sc = p.parse_set_clause().expect("should parse");

        assert_eq!(sc.items.len(), 1);
        match &sc.items[0] {
            SetItem::Property { target, value } => {
                assert_eq!(
                    *target,
                    Expression::Property(
                        Box::new(Expression::Variable("n".to_string())),
                        "name".to_string(),
                    )
                );
                assert_eq!(
                    *value,
                    Expression::Literal(Literal::String("Alice".to_string()))
                );
            }
        }
    }

    #[test]
    fn set_multiple_properties() {
        let (tokens, input) = make_parser("SET n.name = 'Alice', n.age = 30");
        let mut p = Parser::new(&tokens, &input);
        let sc = p.parse_set_clause().expect("should parse");

        assert_eq!(sc.items.len(), 2);
    }

    #[test]
    fn remove_property() {
        let (tokens, input) = make_parser("REMOVE n.email");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_remove_clause().expect("should parse");

        assert_eq!(rc.items.len(), 1);
        match &rc.items[0] {
            RemoveItem::Property(expr) => {
                assert_eq!(
                    *expr,
                    Expression::Property(
                        Box::new(Expression::Variable("n".to_string())),
                        "email".to_string(),
                    )
                );
            }
            _ => panic!("expected property remove"),
        }
    }

    #[test]
    fn remove_label() {
        let (tokens, input) = make_parser("REMOVE n:Person");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_remove_clause().expect("should parse");

        assert_eq!(rc.items.len(), 1);
        match &rc.items[0] {
            RemoveItem::Label { variable, label } => {
                assert_eq!(variable, "n");
                assert_eq!(label, "Person");
            }
            _ => panic!("expected label remove"),
        }
    }

    #[test]
    fn remove_multiple() {
        let (tokens, input) = make_parser("REMOVE n.email, n:Temp");
        let mut p = Parser::new(&tokens, &input);
        let rc = p.parse_remove_clause().expect("should parse");

        assert_eq!(rc.items.len(), 2);
        assert!(matches!(&rc.items[0], RemoveItem::Property(_)));
        assert!(matches!(&rc.items[1], RemoveItem::Label { .. }));
    }

    // ======================================================================
    // TASK-033: parse_delete_clause
    // ======================================================================

    #[test]
    fn delete_single() {
        let (tokens, input) = make_parser("DELETE n");
        let mut p = Parser::new(&tokens, &input);
        let dc = p.parse_delete_clause(false).expect("should parse");

        assert!(!dc.detach);
        assert_eq!(dc.exprs.len(), 1);
        assert_eq!(dc.exprs[0], Expression::Variable("n".to_string()));
    }

    #[test]
    fn delete_multiple() {
        let (tokens, input) = make_parser("DELETE n, m");
        let mut p = Parser::new(&tokens, &input);
        let dc = p.parse_delete_clause(false).expect("should parse");

        assert_eq!(dc.exprs.len(), 2);
    }

    #[test]
    fn delete_detach() {
        let (tokens, input) = make_parser("DELETE n");
        let mut p = Parser::new(&tokens, &input);
        let dc = p.parse_delete_clause(true).expect("should parse");

        assert!(dc.detach);
    }

    // ======================================================================
    // WITH and MERGE (P2)
    // ======================================================================

    #[test]
    fn with_simple() {
        let (tokens, input) = make_parser("WITH n, m");
        let mut p = Parser::new(&tokens, &input);
        let wc = p.parse_with_clause().expect("should parse");

        assert!(!wc.distinct);
        assert_eq!(wc.items.len(), 2);
        assert!(wc.where_clause.is_none());
    }

    #[test]
    fn with_distinct_and_where() {
        let (tokens, input) = make_parser("WITH DISTINCT n WHERE n.age > 30");
        let mut p = Parser::new(&tokens, &input);
        let wc = p.parse_with_clause().expect("should parse");

        assert!(wc.distinct);
        assert_eq!(wc.items.len(), 1);
        assert!(wc.where_clause.is_some());
    }

    #[test]
    fn merge_simple() {
        let (tokens, input) = make_parser("MERGE (n:Person {name: 'Alice'})");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_merge_clause().expect("should parse");

        let node = match &mc.pattern.chains[0].elements[0] {
            PatternElement::Node(n) => n,
            _ => panic!("expected node"),
        };
        assert_eq!(node.variable, Some("n".to_string()));
        assert_eq!(node.labels, vec!["Person".to_string()]);
        assert!(mc.on_match.is_empty());
        assert!(mc.on_create.is_empty());
    }

    // TASK-084: MERGE with ON CREATE SET
    #[test]
    fn merge_on_create_set() {
        let (tokens, input) =
            make_parser("MERGE (n:Person {name: 'Alice'}) ON CREATE SET n.created = true");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_merge_clause().expect("should parse");

        assert!(mc.on_match.is_empty());
        assert_eq!(mc.on_create.len(), 1);
        match &mc.on_create[0] {
            SetItem::Property { target, value } => {
                assert_eq!(
                    *target,
                    Expression::Property(
                        Box::new(Expression::Variable("n".to_string())),
                        "created".to_string(),
                    )
                );
                assert_eq!(*value, Expression::Literal(Literal::Bool(true)));
            }
        }
    }

    // TASK-084: MERGE with ON MATCH SET
    #[test]
    fn merge_on_match_set() {
        let (tokens, input) =
            make_parser("MERGE (n:Person {name: 'Alice'}) ON MATCH SET n.seen = true");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_merge_clause().expect("should parse");

        assert_eq!(mc.on_match.len(), 1);
        assert!(mc.on_create.is_empty());
    }

    // TASK-084: MERGE with both ON CREATE SET and ON MATCH SET
    #[test]
    fn merge_on_create_and_on_match() {
        let (tokens, input) = make_parser(
            "MERGE (n:Person {name: 'Alice'}) ON CREATE SET n.created = true ON MATCH SET n.seen = true",
        );
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_merge_clause().expect("should parse");

        assert_eq!(mc.on_create.len(), 1);
        assert_eq!(mc.on_match.len(), 1);
    }

    // TASK-084: MERGE with multiple SET items in ON CREATE
    #[test]
    fn merge_on_create_multiple_items() {
        let (tokens, input) = make_parser(
            "MERGE (n:Person {name: 'Alice'}) ON CREATE SET n.created = true, n.age = 1",
        );
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_merge_clause().expect("should parse");

        assert_eq!(mc.on_create.len(), 2);
    }

    // ======================================================================
    // X-T3: AT TIME / BETWEEN TIME temporal predicate parsing
    // ======================================================================

    #[test]
    fn match_at_time_literal() {
        let (tokens, input) = make_parser("MATCH (n:Person) AT TIME 1000 WHERE n.age > 30");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert!(mc.temporal_predicate.is_some());
        match mc.temporal_predicate.as_ref().expect("checked above") {
            TemporalPredicate::AsOf(expr) => {
                assert_eq!(*expr, Expression::Literal(Literal::Integer(1000)));
            }
            _ => panic!("expected AsOf temporal predicate"),
        }
        assert!(mc.where_clause.is_some());
    }

    #[test]
    fn match_at_time_function_call() {
        let (tokens, input) =
            make_parser("MATCH (n:Person) AT TIME datetime('2024-01-15T00:00:00Z')");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert!(mc.temporal_predicate.is_some());
        match mc.temporal_predicate.as_ref().expect("checked above") {
            TemporalPredicate::AsOf(Expression::FunctionCall { name, .. }) => {
                assert_eq!(name, "datetime");
            }
            _ => panic!("expected AsOf with datetime function call"),
        }
    }

    #[test]
    fn match_at_time_no_where() {
        let (tokens, input) = make_parser("MATCH (n:Person) AT TIME 1000");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert!(mc.temporal_predicate.is_some());
        assert!(mc.where_clause.is_none());
    }

    #[test]
    fn match_no_temporal_predicate() {
        let (tokens, input) = make_parser("MATCH (n:Person) WHERE n.age > 30");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert!(mc.temporal_predicate.is_none());
        assert!(mc.where_clause.is_some());
    }

    // Y-T1: BETWEEN TIME ... AND ... parsing
    #[test]
    fn match_between_time() {
        let (tokens, input) = make_parser("MATCH (n:Person) BETWEEN TIME 100 AND 200");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert!(mc.temporal_predicate.is_some());
        match mc.temporal_predicate.as_ref().expect("checked above") {
            TemporalPredicate::Between(start, end) => {
                assert_eq!(*start, Expression::Literal(Literal::Integer(100)));
                assert_eq!(*end, Expression::Literal(Literal::Integer(200)));
            }
            _ => panic!("expected Between temporal predicate"),
        }
    }

    #[test]
    fn match_between_time_with_where() {
        let (tokens, input) =
            make_parser("MATCH (n:Person) BETWEEN TIME 100 AND 200 WHERE n.age > 30");
        let mut p = Parser::new(&tokens, &input);
        let mc = p.parse_match_clause(false).expect("should parse");

        assert!(mc.temporal_predicate.is_some());
        assert!(matches!(
            mc.temporal_predicate.as_ref().expect("checked"),
            TemporalPredicate::Between(_, _)
        ));
        assert!(mc.where_clause.is_some());
    }

    // ======================================================================
    // TASK-068: parse_unwind_clause
    // ======================================================================

    #[test]
    fn unwind_list_literal() {
        let (tokens, input) = make_parser("UNWIND [1, 2, 3] AS x");
        let mut p = Parser::new(&tokens, &input);
        let uc = p.parse_unwind_clause().expect("should parse");

        assert_eq!(
            uc.expr,
            Expression::ListLiteral(vec![
                Expression::Literal(Literal::Integer(1)),
                Expression::Literal(Literal::Integer(2)),
                Expression::Literal(Literal::Integer(3)),
            ])
        );
        assert_eq!(uc.variable, "x");
    }

    #[test]
    fn unwind_variable_expression() {
        let (tokens, input) = make_parser("UNWIND items AS item");
        let mut p = Parser::new(&tokens, &input);
        let uc = p.parse_unwind_clause().expect("should parse");

        assert_eq!(uc.expr, Expression::Variable("items".to_string()));
        assert_eq!(uc.variable, "item");
    }

    #[test]
    fn unwind_property_expression() {
        let (tokens, input) = make_parser("UNWIND n.hobbies AS h");
        let mut p = Parser::new(&tokens, &input);
        let uc = p.parse_unwind_clause().expect("should parse");

        assert_eq!(
            uc.expr,
            Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "hobbies".to_string(),
            )
        );
        assert_eq!(uc.variable, "h");
    }

    #[test]
    fn unwind_empty_list() {
        let (tokens, input) = make_parser("UNWIND [] AS x");
        let mut p = Parser::new(&tokens, &input);
        let uc = p.parse_unwind_clause().expect("should parse");

        assert_eq!(uc.expr, Expression::ListLiteral(vec![]));
        assert_eq!(uc.variable, "x");
    }
}
