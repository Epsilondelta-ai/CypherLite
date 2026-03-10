// Parser module: recursive descent parser producing AST
pub mod ast;
pub mod clause;
pub mod expression;
pub mod pattern;

use crate::lexer::{lex, LexError, Span, Token};
pub use ast::*;

// @MX:ANCHOR: [AUTO] Main entry point for the query pipeline — called by SemanticAnalyzer, Planner, and API layer
// @MX:REASON: fan_in >= 3; all query processing starts here
/// Parse a Cypher query string into a `Query` AST (TASK-034).
///
/// This is the main public entry point for the parser. It lexes the input,
/// then dispatches to the appropriate clause parsers based on the leading
/// keyword token.
pub fn parse_query(input: &str) -> Result<Query, ParseError> {
    let tokens = lex(input).map_err(|e: LexError| {
        // Convert byte offset to line/col for consistent error reporting
        let mut line = 1;
        let mut col = 1;
        for (i, ch) in input.char_indices() {
            if i >= e.position {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        ParseError {
            line,
            column: col,
            message: e.to_string(),
        }
    })?;

    let mut parser = Parser::new(&tokens, input);
    let mut clauses = Vec::new();

    while !parser.at_end() {
        let clause = match parser.peek() {
            Some(Token::Optional) => {
                parser.advance(); // consume OPTIONAL
                if !parser.check(&Token::Match) {
                    return Err(parser.error("expected MATCH after OPTIONAL"));
                }
                Clause::Match(parser.parse_match_clause(true)?)
            }
            Some(Token::Match) => Clause::Match(parser.parse_match_clause(false)?),
            Some(Token::Return) => Clause::Return(parser.parse_return_clause()?),
            Some(Token::Create) => {
                // Peek at next token to distinguish CREATE INDEX from CREATE (pattern)
                if parser.tokens.get(parser.pos + 1).map(|(t, _)| t) == Some(&Token::Index) {
                    parser.advance(); // consume CREATE
                    Clause::CreateIndex(parser.parse_create_index_clause()?)
                } else {
                    Clause::Create(parser.parse_create_clause()?)
                }
            }
            Some(Token::Set) => Clause::Set(parser.parse_set_clause()?),
            Some(Token::Remove) => Clause::Remove(parser.parse_remove_clause()?),
            Some(Token::Delete) => Clause::Delete(parser.parse_delete_clause(false)?),
            Some(Token::Detach) => {
                parser.advance(); // consume DETACH
                if !parser.check(&Token::Delete) {
                    return Err(parser.error("expected DELETE after DETACH"));
                }
                Clause::Delete(parser.parse_delete_clause(true)?)
            }
            Some(Token::With) => Clause::With(parser.parse_with_clause()?),
            Some(Token::Merge) => Clause::Merge(parser.parse_merge_clause()?),
            Some(Token::Unwind) => Clause::Unwind(parser.parse_unwind_clause()?),
            Some(Token::Drop) => {
                Clause::DropIndex(parser.parse_drop_index_clause()?)
            }
            Some(Token::Where) => {
                return Err(parser.error("WHERE clause must follow a MATCH clause"));
            }
            Some(Token::Order | Token::Limit | Token::Skip) => {
                return Err(parser.error("ORDER BY / SKIP / LIMIT must be part of a RETURN clause"));
            }
            Some(tok) => {
                return Err(parser.error(format!("unexpected token at top level: {:?}", tok)));
            }
            None => break,
        };
        clauses.push(clause);
    }

    if clauses.is_empty() {
        return Err(ParseError {
            line: 1,
            column: 1,
            message: "empty query".to_string(),
        });
    }

    Ok(Query { clauses })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parse error at {}:{}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Recursive descent parser for the openCypher subset.
pub struct Parser<'a> {
    tokens: &'a [(Token, Span)],
    pos: usize,
    input: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [(Token, Span)], input: &'a str) -> Self {
        Self {
            tokens,
            pos: 0,
            input,
        }
    }

    /// Peek at the current token without consuming it.
    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    /// Advance the parser by one token, returning the consumed token and span.
    pub fn advance(&mut self) -> Option<&(Token, Span)> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    /// Expect a specific token, consuming it. Returns the span on success.
    pub fn expect(&mut self, expected: &Token) -> Result<Span, ParseError> {
        match self.tokens.get(self.pos) {
            Some((tok, span)) if tok == expected => {
                let s = *span;
                self.pos += 1;
                Ok(s)
            }
            Some((tok, span)) => {
                let (line, col) = self.offset_to_line_col(span.start);
                Err(ParseError {
                    line,
                    column: col,
                    message: format!("expected {:?}, found {:?}", expected, tok),
                })
            }
            None => {
                let offset = self.tokens.last().map(|(_, s)| s.end).unwrap_or(0);
                let (line, col) = self.offset_to_line_col(offset);
                Err(ParseError {
                    line,
                    column: col,
                    message: format!("expected {:?}, found end of input", expected),
                })
            }
        }
    }

    /// Check if the current token matches without consuming it.
    pub fn check(&self, expected: &Token) -> bool {
        self.peek() == Some(expected)
    }

    /// Consume the current token if it matches, returning true.
    pub fn eat(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Return the span of the current token, or a zero-length span at EOF.
    pub fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|(_, s)| *s)
            .unwrap_or(Span { start: 0, end: 0 })
    }

    /// Convert byte offset to (line, column) using the input string.
    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;
        for (i, ch) in self.input.char_indices() {
            if i >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Return a `ParseError` at the current position with the given message.
    pub fn error(&self, message: impl Into<String>) -> ParseError {
        let span = self.current_span();
        let (line, col) = self.offset_to_line_col(span.start);
        ParseError {
            line,
            column: col,
            message: message.into(),
        }
    }

    /// Return true when there are no more tokens.
    pub fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Expect and consume an identifier, returning its name.
    pub fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.tokens.get(self.pos) {
            Some((Token::Ident(name), _)) => {
                let name = name.clone();
                self.pos += 1;
                Ok(name)
            }
            Some((Token::BacktickIdent(name), _)) => {
                let name = name.clone();
                self.pos += 1;
                Ok(name)
            }
            Some((tok, span)) => {
                let (line, col) = self.offset_to_line_col(span.start);
                Err(ParseError {
                    line,
                    column: col,
                    message: format!("expected identifier, found {:?}", tok),
                })
            }
            None => {
                let offset = self.tokens.last().map(|(_, s)| s.end).unwrap_or(0);
                let (line, col) = self.offset_to_line_col(offset);
                Err(ParseError {
                    line,
                    column: col,
                    message: "expected identifier, found end of input".to_string(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Span;

    #[test]
    fn parser_peek_and_advance() {
        let tokens = lex("MATCH").expect("should lex");
        let mut p = Parser::new(&tokens, "MATCH");
        assert_eq!(p.peek(), Some(&Token::Match));
        let tok = p.advance();
        assert!(tok.is_some());
        assert_eq!(p.peek(), None);
    }

    #[test]
    fn parser_expect_success() {
        let tokens = lex("(").expect("should lex");
        let mut p = Parser::new(&tokens, "(");
        let span = p.expect(&Token::LParen);
        assert!(span.is_ok());
        assert_eq!(span.expect("checked above"), Span { start: 0, end: 1 });
    }

    #[test]
    fn parser_expect_failure() {
        let tokens = lex("(").expect("should lex");
        let mut p = Parser::new(&tokens, "(");
        let result = p.expect(&Token::RParen);
        assert!(result.is_err());
    }

    #[test]
    fn parser_offset_to_line_col() {
        let input = "line1\nline2\nline3";
        let tokens = lex(input).expect("should lex");
        let p = Parser::new(&tokens, input);
        assert_eq!(p.offset_to_line_col(0), (1, 1));
        assert_eq!(p.offset_to_line_col(6), (2, 1));
        assert_eq!(p.offset_to_line_col(12), (3, 1));
    }

    #[test]
    fn parser_eat_and_check() {
        let tokens = lex("( )").expect("should lex");
        let mut p = Parser::new(&tokens, "( )");
        assert!(p.check(&Token::LParen));
        assert!(!p.check(&Token::RParen));
        assert!(p.eat(&Token::LParen));
        assert!(!p.eat(&Token::LParen));
        assert!(p.eat(&Token::RParen));
        assert!(p.at_end());
    }

    #[test]
    fn parser_expect_ident() {
        let tokens = lex("foo").expect("should lex");
        let mut p = Parser::new(&tokens, "foo");
        let name = p.expect_ident();
        assert_eq!(name.expect("should be ident"), "foo");
    }

    #[test]
    fn parser_expect_ident_backtick() {
        let tokens = lex("`my var`").expect("should lex");
        let mut p = Parser::new(&tokens, "`my var`");
        let name = p.expect_ident();
        assert_eq!(name.expect("should be ident"), "my var");
    }

    #[test]
    fn parse_error_display() {
        let err = ParseError {
            line: 1,
            column: 5,
            message: "unexpected token".to_string(),
        };
        assert_eq!(err.to_string(), "Parse error at 1:5: unexpected token");
    }

    // ======================================================================
    // TASK-035: Integration tests -- full query round-trip
    // ======================================================================

    #[test]
    fn query_match_return() {
        let q = parse_query("MATCH (n:Person) RETURN n").expect("should parse");
        assert_eq!(q.clauses.len(), 2);
        assert!(matches!(&q.clauses[0], Clause::Match(_)));
        assert!(matches!(&q.clauses[1], Clause::Return(_)));

        if let Clause::Match(mc) = &q.clauses[0] {
            assert!(!mc.optional);
            let node = match &mc.pattern.chains[0].elements[0] {
                PatternElement::Node(n) => n,
                _ => panic!("expected node"),
            };
            assert_eq!(node.labels, vec!["Person".to_string()]);
        }
    }

    #[test]
    fn query_create_with_properties() {
        let q = parse_query("CREATE (n:Person {name: 'Alice'})").expect("should parse");
        assert_eq!(q.clauses.len(), 1);
        assert!(matches!(&q.clauses[0], Clause::Create(_)));

        if let Clause::Create(cc) = &q.clauses[0] {
            let node = match &cc.pattern.chains[0].elements[0] {
                PatternElement::Node(n) => n,
                _ => panic!("expected node"),
            };
            assert_eq!(node.labels, vec!["Person".to_string()]);
            assert!(node.properties.is_some());
        }
    }

    #[test]
    fn query_match_relationship_return() {
        let q = parse_query("MATCH (a)-[:KNOWS]->(b) RETURN b.name").expect("should parse");
        assert_eq!(q.clauses.len(), 2);

        if let Clause::Match(mc) = &q.clauses[0] {
            assert_eq!(mc.pattern.chains[0].elements.len(), 3);
        } else {
            panic!("expected MATCH clause");
        }

        if let Clause::Return(rc) = &q.clauses[1] {
            assert_eq!(
                rc.items[0].expr,
                Expression::Property(
                    Box::new(Expression::Variable("b".to_string())),
                    "name".to_string(),
                )
            );
        } else {
            panic!("expected RETURN clause");
        }
    }

    #[test]
    fn query_match_where_return() {
        let q =
            parse_query("MATCH (n:Person) WHERE n.age > 30 RETURN n.name").expect("should parse");
        assert_eq!(q.clauses.len(), 2);

        if let Clause::Match(mc) = &q.clauses[0] {
            assert!(mc.where_clause.is_some());
        } else {
            panic!("expected MATCH clause");
        }
    }

    #[test]
    fn query_return_order_limit() {
        let q = parse_query("MATCH (n:Person) RETURN n.name ORDER BY n.name ASC LIMIT 10")
            .expect("should parse");
        assert_eq!(q.clauses.len(), 2);

        if let Clause::Return(rc) = &q.clauses[1] {
            let order = rc.order_by.as_ref().expect("should have ORDER BY");
            assert_eq!(order.len(), 1);
            assert!(order[0].ascending);
            assert_eq!(
                rc.limit.as_ref().expect("should have LIMIT"),
                &Expression::Literal(Literal::Integer(10))
            );
        } else {
            panic!("expected RETURN clause");
        }
    }

    #[test]
    fn query_match_set() {
        let q = parse_query("MATCH (n:Person) SET n.age = 30").expect("should parse");
        assert_eq!(q.clauses.len(), 2);
        assert!(matches!(&q.clauses[0], Clause::Match(_)));
        assert!(matches!(&q.clauses[1], Clause::Set(_)));
    }

    #[test]
    fn query_detach_delete() {
        let q = parse_query("DETACH DELETE n").expect("should parse");
        assert_eq!(q.clauses.len(), 1);

        if let Clause::Delete(dc) = &q.clauses[0] {
            assert!(dc.detach);
            assert_eq!(dc.exprs[0], Expression::Variable("n".to_string()));
        } else {
            panic!("expected DELETE clause");
        }
    }

    #[test]
    fn query_optional_match() {
        let q = parse_query("MATCH (n:Person) OPTIONAL MATCH (n)-[:KNOWS]->(m) RETURN n, m")
            .expect("should parse");
        assert_eq!(q.clauses.len(), 3);

        if let Clause::Match(mc) = &q.clauses[0] {
            assert!(!mc.optional);
        } else {
            panic!("expected MATCH clause");
        }

        if let Clause::Match(mc) = &q.clauses[1] {
            assert!(mc.optional);
        } else {
            panic!("expected OPTIONAL MATCH clause");
        }
    }

    #[test]
    fn query_count_star() {
        let q = parse_query("MATCH (n) RETURN count(*)").expect("should parse");
        assert_eq!(q.clauses.len(), 2);

        if let Clause::Return(rc) = &q.clauses[1] {
            assert_eq!(rc.items[0].expr, Expression::CountStar);
        } else {
            panic!("expected RETURN clause");
        }
    }

    #[test]
    fn query_count_distinct() {
        let q = parse_query("MATCH (n) RETURN count(DISTINCT n.name)").expect("should parse");

        if let Clause::Return(rc) = &q.clauses[1] {
            assert_eq!(
                rc.items[0].expr,
                Expression::FunctionCall {
                    name: "count".to_string(),
                    distinct: true,
                    args: vec![Expression::Property(
                        Box::new(Expression::Variable("n".to_string())),
                        "name".to_string(),
                    )],
                }
            );
        } else {
            panic!("expected RETURN clause");
        }
    }

    #[test]
    fn query_create_relationship() {
        let q = parse_query("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
            .expect("should parse");
        assert_eq!(q.clauses.len(), 1);

        if let Clause::Create(cc) = &q.clauses[0] {
            assert_eq!(cc.pattern.chains[0].elements.len(), 3);
            // Verify node a
            if let PatternElement::Node(n) = &cc.pattern.chains[0].elements[0] {
                assert_eq!(n.variable, Some("a".to_string()));
                assert_eq!(n.labels, vec!["Person".to_string()]);
            }
            // Verify relationship
            if let PatternElement::Relationship(r) = &cc.pattern.chains[0].elements[1] {
                assert_eq!(r.rel_types, vec!["KNOWS".to_string()]);
                assert_eq!(r.direction, RelDirection::Outgoing);
            }
            // Verify node b
            if let PatternElement::Node(n) = &cc.pattern.chains[0].elements[2] {
                assert_eq!(n.variable, Some("b".to_string()));
            }
        } else {
            panic!("expected CREATE clause");
        }
    }

    // -- Error cases --

    #[test]
    fn query_error_empty() {
        let result = parse_query("");
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .message
            .contains("empty query"));
    }

    #[test]
    fn query_error_where_without_match() {
        let result = parse_query("WHERE n.age > 30");
        assert!(result.is_err());
        assert!(result.expect_err("should fail").message.contains("WHERE"));
    }

    #[test]
    fn query_error_order_without_return() {
        let result = parse_query("ORDER BY n.name");
        assert!(result.is_err());
    }

    #[test]
    fn query_error_unexpected_token() {
        let result = parse_query("42");
        assert!(result.is_err());
    }

    #[test]
    fn query_error_lex_error() {
        let result = parse_query("MATCH @");
        assert!(result.is_err());
    }

    #[test]
    fn query_with_clause() {
        let q = parse_query("MATCH (n) WITH n WHERE n.age > 30 RETURN n").expect("should parse");
        assert_eq!(q.clauses.len(), 3);
        assert!(matches!(&q.clauses[0], Clause::Match(_)));
        assert!(matches!(&q.clauses[1], Clause::With(_)));
        assert!(matches!(&q.clauses[2], Clause::Return(_)));

        if let Clause::With(wc) = &q.clauses[1] {
            assert!(wc.where_clause.is_some());
        }
    }

    #[test]
    fn query_merge() {
        let q = parse_query("MERGE (n:Person {name: 'Alice'})").expect("should parse");
        assert_eq!(q.clauses.len(), 1);
        assert!(matches!(&q.clauses[0], Clause::Merge(_)));
    }

    #[test]
    fn query_match_remove() {
        let q = parse_query("MATCH (n:Person) REMOVE n.email, n:Temp").expect("should parse");
        assert_eq!(q.clauses.len(), 2);
        assert!(matches!(&q.clauses[1], Clause::Remove(_)));
    }

    #[test]
    fn query_match_delete() {
        let q = parse_query("MATCH (n:Person) DELETE n").expect("should parse");
        assert_eq!(q.clauses.len(), 2);

        if let Clause::Delete(dc) = &q.clauses[1] {
            assert!(!dc.detach);
        }
    }

    #[test]
    fn query_return_skip_limit() {
        let q = parse_query("MATCH (n) RETURN n SKIP 5 LIMIT 10").expect("should parse");

        if let Clause::Return(rc) = &q.clauses[1] {
            assert_eq!(
                rc.skip.as_ref().expect("should have SKIP"),
                &Expression::Literal(Literal::Integer(5))
            );
            assert_eq!(
                rc.limit.as_ref().expect("should have LIMIT"),
                &Expression::Literal(Literal::Integer(10))
            );
        }
    }

    #[test]
    fn query_case_insensitive() {
        let q = parse_query("match (n:Person) return n").expect("should parse");
        assert_eq!(q.clauses.len(), 2);
    }

    // ======================================================================
    // TASK-098: CREATE INDEX / DROP INDEX parsing
    // ======================================================================

    #[test]
    fn query_create_index_with_name() {
        let q = parse_query("CREATE INDEX idx_person_name ON :Person(name)").expect("should parse");
        assert_eq!(q.clauses.len(), 1);

        if let Clause::CreateIndex(ci) = &q.clauses[0] {
            assert_eq!(ci.name, Some("idx_person_name".to_string()));
            assert_eq!(ci.label, "Person");
            assert_eq!(ci.property, "name");
        } else {
            panic!("expected CreateIndex clause");
        }
    }

    #[test]
    fn query_create_index_without_name() {
        let q = parse_query("CREATE INDEX ON :Person(name)").expect("should parse");
        assert_eq!(q.clauses.len(), 1);

        if let Clause::CreateIndex(ci) = &q.clauses[0] {
            assert_eq!(ci.name, None);
            assert_eq!(ci.label, "Person");
            assert_eq!(ci.property, "name");
        } else {
            panic!("expected CreateIndex clause");
        }
    }

    #[test]
    fn query_drop_index() {
        let q = parse_query("DROP INDEX idx_person_name").expect("should parse");
        assert_eq!(q.clauses.len(), 1);

        if let Clause::DropIndex(di) = &q.clauses[0] {
            assert_eq!(di.name, "idx_person_name");
        } else {
            panic!("expected DropIndex clause");
        }
    }

    // ======================================================================
    // TASK-068: UNWIND clause integration tests
    // ======================================================================

    #[test]
    fn query_unwind_list_return() {
        let q = parse_query("UNWIND [1, 2, 3] AS x RETURN x").expect("should parse");
        assert_eq!(q.clauses.len(), 2);
        assert!(matches!(&q.clauses[0], Clause::Unwind(_)));
        assert!(matches!(&q.clauses[1], Clause::Return(_)));

        if let Clause::Unwind(uc) = &q.clauses[0] {
            assert_eq!(uc.variable, "x");
            assert!(matches!(&uc.expr, Expression::ListLiteral(_)));
        }
    }

    #[test]
    fn query_match_unwind_return() {
        let q = parse_query("MATCH (n:Person) UNWIND n.hobbies AS h RETURN h")
            .expect("should parse");
        assert_eq!(q.clauses.len(), 3);
        assert!(matches!(&q.clauses[0], Clause::Match(_)));
        assert!(matches!(&q.clauses[1], Clause::Unwind(_)));
        assert!(matches!(&q.clauses[2], Clause::Return(_)));
    }
}
