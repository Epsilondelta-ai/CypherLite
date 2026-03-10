// Pattern parsing: node patterns, relationship patterns, path chains

use super::ast::*;
use super::{ParseError, Parser};
use crate::lexer::Token;

impl<'a> Parser<'a> {
    /// Parse a full pattern: `node (rel node)* (, node (rel node)*)*`
    pub fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let mut chains = Vec::new();
        chains.push(self.parse_pattern_chain()?);
        while self.eat(&Token::Comma) {
            chains.push(self.parse_pattern_chain()?);
        }
        Ok(Pattern { chains })
    }

    /// Parse a single pattern chain: `node (rel node)*`
    fn parse_pattern_chain(&mut self) -> Result<PatternChain, ParseError> {
        let mut elements = Vec::new();
        elements.push(PatternElement::Node(self.parse_node_pattern()?));

        loop {
            match self.peek() {
                // Outgoing or undirected: starts with `-`
                Some(Token::Minus) => {
                    let rel = self.parse_relationship_from_minus()?;
                    elements.push(PatternElement::Relationship(rel));
                    elements.push(PatternElement::Node(self.parse_node_pattern()?));
                }
                // Incoming: starts with `<-`
                Some(Token::ArrowLeft) => {
                    let rel = self.parse_relationship_incoming()?;
                    elements.push(PatternElement::Relationship(rel));
                    elements.push(PatternElement::Node(self.parse_node_pattern()?));
                }
                // Undirected without brackets: `--`
                Some(Token::DoubleDash) => {
                    self.advance(); // consume --
                    elements.push(PatternElement::Relationship(RelationshipPattern {
                        variable: None,
                        rel_types: Vec::new(),
                        direction: RelDirection::Undirected,
                        properties: None,
                        min_hops: None,
                        max_hops: None,
                    }));
                    elements.push(PatternElement::Node(self.parse_node_pattern()?));
                }
                _ => break,
            }
        }

        Ok(PatternChain { elements })
    }

    /// Parse a node pattern: `(variable:Label:Label2 {prop: value, ...})`
    /// All parts are optional except the parentheses.
    pub fn parse_node_pattern(&mut self) -> Result<NodePattern, ParseError> {
        self.expect(&Token::LParen)?;

        let mut variable = None;
        let mut labels = Vec::new();
        let mut properties = None;

        // Optional variable name (identifier not followed by colon could still be
        // a variable, or identifier followed by colon means variable + label).
        if let Some(Token::Ident(_) | Token::BacktickIdent(_)) = self.peek() {
            variable = Some(self.expect_ident()?);
        }

        // Optional labels (one or more `:Label`)
        while self.eat(&Token::Colon) {
            labels.push(self.expect_ident()?);
        }

        // Optional properties `{key: value, ...}`
        if self.check(&Token::LBrace) {
            properties = Some(self.parse_map_literal()?);
        }

        self.expect(&Token::RParen)?;

        Ok(NodePattern {
            variable,
            labels,
            properties,
        })
    }

    /// Parse a map literal: `{key: value, key: value, ...}`
    pub fn parse_map_literal(&mut self) -> Result<MapLiteral, ParseError> {
        self.expect(&Token::LBrace)?;
        let mut entries = Vec::new();

        if !self.check(&Token::RBrace) {
            let key = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let value = self.parse_expression()?;
            entries.push((key, value));

            while self.eat(&Token::Comma) {
                let key = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let value = self.parse_expression()?;
                entries.push((key, value));
            }
        }

        self.expect(&Token::RBrace)?;
        Ok(entries)
    }

    /// Parse relationship starting with `-` (outgoing `-[...]->`  or undirected `-[...]-`).
    fn parse_relationship_from_minus(&mut self) -> Result<RelationshipPattern, ParseError> {
        self.expect(&Token::Minus)?;

        // Must have `[` for bracket content
        self.expect(&Token::LBracket)?;

        // Check for variable-length path `*` before or after content
        let content = self.parse_relationship_content()?;

        self.expect(&Token::RBracket)?;

        // Determine direction: `->` means outgoing, `-` means undirected
        let direction = if self.eat(&Token::ArrowRight) {
            RelDirection::Outgoing
        } else if self.eat(&Token::Minus) {
            RelDirection::Undirected
        } else {
            return Err(self.error("expected -> or - after relationship bracket"));
        };

        Ok(RelationshipPattern {
            variable: content.variable,
            rel_types: content.rel_types,
            direction,
            properties: content.properties,
            min_hops: content.min_hops,
            max_hops: content.max_hops,
        })
    }

    /// Parse incoming relationship: `<-[...]-`
    fn parse_relationship_incoming(&mut self) -> Result<RelationshipPattern, ParseError> {
        // Consume `<-`
        self.expect(&Token::ArrowLeft)?;

        self.expect(&Token::LBracket)?;
        let content = self.parse_relationship_content()?;
        self.expect(&Token::RBracket)?;

        // Must end with `-`
        self.expect(&Token::Minus)?;

        Ok(RelationshipPattern {
            variable: content.variable,
            rel_types: content.rel_types,
            direction: RelDirection::Incoming,
            properties: content.properties,
            min_hops: content.min_hops,
            max_hops: content.max_hops,
        })
    }

    /// Parse content inside relationship brackets: `[variable :TYPE | TYPE2 *N..M {props}]`
    fn parse_relationship_content(&mut self) -> Result<RelContentResult, ParseError> {
        let mut variable = None;
        let mut rel_types = Vec::new();
        let mut properties = None;
        let mut min_hops = None;
        let mut max_hops = None;

        // Check for bare star: [*...] (no variable, no type)
        if self.check(&Token::Star) {
            let (mn, mx) = self.parse_var_length_spec()?;
            min_hops = Some(mn);
            max_hops = mx;

            // After star spec, optional properties then done
            if self.check(&Token::LBrace) {
                properties = Some(self.parse_map_literal()?);
            }
            return Ok(RelContentResult {
                variable,
                rel_types,
                properties,
                min_hops,
                max_hops,
            });
        }

        // Optional variable
        if let Some(Token::Ident(_) | Token::BacktickIdent(_)) = self.peek() {
            variable = Some(self.expect_ident()?);
        }

        // Optional variable-length path star after variable: [r*...]
        if self.check(&Token::Star) {
            let (mn, mx) = self.parse_var_length_spec()?;
            min_hops = Some(mn);
            max_hops = mx;

            if self.check(&Token::LBrace) {
                properties = Some(self.parse_map_literal()?);
            }
            return Ok(RelContentResult {
                variable,
                rel_types,
                properties,
                min_hops,
                max_hops,
            });
        }

        // Optional relationship types: `:TYPE` or `:TYPE|TYPE2`
        if self.eat(&Token::Colon) {
            rel_types.push(self.expect_ident()?);
            while self.eat(&Token::Pipe) {
                rel_types.push(self.expect_ident()?);
            }
        }

        // Optional variable-length path star after types: [:TYPE*...]
        if self.check(&Token::Star) {
            let (mn, mx) = self.parse_var_length_spec()?;
            min_hops = Some(mn);
            max_hops = mx;
        }

        // Optional properties
        if self.check(&Token::LBrace) {
            properties = Some(self.parse_map_literal()?);
        }

        Ok(RelContentResult {
            variable,
            rel_types,
            properties,
            min_hops,
            max_hops,
        })
    }

    /// Parse variable-length spec after `*`: `*`, `*N`, `*N..M`, `*N..`, `*..M`
    /// Returns (min_hops, max_hops).
    fn parse_var_length_spec(&mut self) -> Result<(u32, Option<u32>), ParseError> {
        self.expect(&Token::Star)?;

        // Check for DoubleDot immediately: *..M
        if self.eat(&Token::DoubleDot) {
            // *..M form
            if let Some(Token::Integer(n)) = self.peek() {
                let max = Self::int_to_u32(*n, self)?;
                self.advance();
                return Ok((1, Some(max)));
            }
            // *.. alone (unbounded from 1)
            return Ok((1, None));
        }

        // Check for integer: *N or *N..M or *N..
        if let Some(Token::Integer(n)) = self.peek() {
            let first = Self::int_to_u32(*n, self)?;
            self.advance();

            // Check for DoubleDot: *N..M or *N..
            if self.eat(&Token::DoubleDot) {
                if let Some(Token::Integer(m)) = self.peek() {
                    let second = Self::int_to_u32(*m, self)?;
                    self.advance();
                    return Ok((first, Some(second)));
                }
                // *N.. (open end)
                return Ok((first, None));
            }

            // *N (exact hop)
            return Ok((first, Some(first)));
        }

        // Just * alone (unbounded)
        Ok((1, None))
    }

    /// Convert i64 to u32 for hop counts, returning error if negative.
    fn int_to_u32(n: i64, parser: &Self) -> Result<u32, ParseError> {
        if n < 0 {
            return Err(parser.error("hop count must be non-negative"));
        }
        Ok(n as u32)
    }
}

/// Intermediate result for relationship bracket content.
struct RelContentResult {
    variable: Option<String>,
    rel_types: Vec<String>,
    properties: Option<MapLiteral>,
    min_hops: Option<u32>,
    max_hops: Option<u32>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    /// Helper: parse a pattern from a string.
    fn parse_pattern_str(input: &str) -> Result<Pattern, ParseError> {
        let tokens = lex(input).expect("lexing should succeed");
        let mut parser = Parser::new(&tokens, input);
        parser.parse_pattern()
    }

    /// Helper: parse a node pattern from a string.
    fn parse_node(input: &str) -> Result<NodePattern, ParseError> {
        let tokens = lex(input).expect("lexing should succeed");
        let mut parser = Parser::new(&tokens, input);
        parser.parse_node_pattern()
    }

    // -- TASK-027: Pattern parser tests --

    // Single node: (n:Person)
    #[test]
    fn pattern_single_node_with_label() {
        let node = parse_node("(n:Person)").expect("should parse");
        assert_eq!(
            node,
            NodePattern {
                variable: Some("n".to_string()),
                labels: vec!["Person".to_string()],
                properties: None,
            }
        );
    }

    // Node with no label: (n)
    #[test]
    fn pattern_node_no_label() {
        let node = parse_node("(n)").expect("should parse");
        assert_eq!(
            node,
            NodePattern {
                variable: Some("n".to_string()),
                labels: vec![],
                properties: None,
            }
        );
    }

    // Empty node: ()
    #[test]
    fn pattern_empty_node() {
        let node = parse_node("()").expect("should parse");
        assert_eq!(
            node,
            NodePattern {
                variable: None,
                labels: vec![],
                properties: None,
            }
        );
    }

    // Node with properties: (n:Person {name: 'Alice'})
    #[test]
    fn pattern_node_with_properties() {
        let node = parse_node("(n:Person {name: 'Alice'})").expect("should parse");
        assert_eq!(
            node,
            NodePattern {
                variable: Some("n".to_string()),
                labels: vec!["Person".to_string()],
                properties: Some(vec![(
                    "name".to_string(),
                    Expression::Literal(Literal::String("Alice".to_string())),
                )]),
            }
        );
    }

    // Node with multiple labels: (n:Person:Employee)
    #[test]
    fn pattern_node_multiple_labels() {
        let node = parse_node("(n:Person:Employee)").expect("should parse");
        assert_eq!(
            node,
            NodePattern {
                variable: Some("n".to_string()),
                labels: vec!["Person".to_string(), "Employee".to_string()],
                properties: None,
            }
        );
    }

    // Node with multiple properties
    #[test]
    fn pattern_node_multiple_properties() {
        let node = parse_node("(n:Person {name: 'Alice', age: 30})").expect("should parse");
        assert_eq!(
            node,
            NodePattern {
                variable: Some("n".to_string()),
                labels: vec!["Person".to_string()],
                properties: Some(vec![
                    (
                        "name".to_string(),
                        Expression::Literal(Literal::String("Alice".to_string())),
                    ),
                    ("age".to_string(), Expression::Literal(Literal::Integer(30)),),
                ]),
            }
        );
    }

    // Label without variable: (:Person)
    #[test]
    fn pattern_node_label_no_variable() {
        let node = parse_node("(:Person)").expect("should parse");
        assert_eq!(
            node,
            NodePattern {
                variable: None,
                labels: vec!["Person".to_string()],
                properties: None,
            }
        );
    }

    // Outgoing relationship: (a)-[:KNOWS]->(b)
    #[test]
    fn pattern_outgoing_relationship() {
        let pattern = parse_pattern_str("(a)-[:KNOWS]->(b)").expect("should parse");
        assert_eq!(pattern.chains.len(), 1);
        let chain = &pattern.chains[0];
        assert_eq!(chain.elements.len(), 3);

        assert_eq!(
            chain.elements[0],
            PatternElement::Node(NodePattern {
                variable: Some("a".to_string()),
                labels: vec![],
                properties: None,
            })
        );
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: None,
                rel_types: vec!["KNOWS".to_string()],
                direction: RelDirection::Outgoing,
                properties: None,
                min_hops: None,
                max_hops: None,
            })
        );
        assert_eq!(
            chain.elements[2],
            PatternElement::Node(NodePattern {
                variable: Some("b".to_string()),
                labels: vec![],
                properties: None,
            })
        );
    }

    // Incoming: (a)<-[:KNOWS]-(b)
    #[test]
    fn pattern_incoming_relationship() {
        let pattern = parse_pattern_str("(a)<-[:KNOWS]-(b)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: None,
                rel_types: vec!["KNOWS".to_string()],
                direction: RelDirection::Incoming,
                properties: None,
                min_hops: None,
                max_hops: None,
            })
        );
    }

    // Undirected with brackets: (a)-[:KNOWS]-(b)
    #[test]
    fn pattern_undirected_relationship() {
        let pattern = parse_pattern_str("(a)-[:KNOWS]-(b)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: None,
                rel_types: vec!["KNOWS".to_string()],
                direction: RelDirection::Undirected,
                properties: None,
                min_hops: None,
                max_hops: None,
            })
        );
    }

    // Undirected without brackets: (a)--(b)
    #[test]
    fn pattern_undirected_no_brackets() {
        let pattern = parse_pattern_str("(a)--(b)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: None,
                rel_types: vec![],
                direction: RelDirection::Undirected,
                properties: None,
                min_hops: None,
                max_hops: None,
            })
        );
    }

    // Relationship with variable: (a)-[r:KNOWS]->(b)
    #[test]
    fn pattern_relationship_with_variable() {
        let pattern = parse_pattern_str("(a)-[r:KNOWS]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: Some("r".to_string()),
                rel_types: vec!["KNOWS".to_string()],
                direction: RelDirection::Outgoing,
                properties: None,
                min_hops: None,
                max_hops: None,
            })
        );
    }

    // Relationship with properties: (a)-[r:KNOWS {since: 2020}]->(b)
    #[test]
    fn pattern_relationship_with_properties() {
        let pattern = parse_pattern_str("(a)-[r:KNOWS {since: 2020}]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: Some("r".to_string()),
                rel_types: vec!["KNOWS".to_string()],
                direction: RelDirection::Outgoing,
                properties: Some(vec![(
                    "since".to_string(),
                    Expression::Literal(Literal::Integer(2020)),
                )]),
                min_hops: None,
                max_hops: None,
            })
        );
    }

    // Multiple relationship types: (a)-[:KNOWS|LIKES]->(b)
    #[test]
    fn pattern_multiple_rel_types() {
        let pattern = parse_pattern_str("(a)-[:KNOWS|LIKES]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: None,
                rel_types: vec!["KNOWS".to_string(), "LIKES".to_string()],
                direction: RelDirection::Outgoing,
                properties: None,
                min_hops: None,
                max_hops: None,
            })
        );
    }

    // Multi-hop: (a)-[:KNOWS]->(b)-[:KNOWS]->(c)
    #[test]
    fn pattern_multi_hop() {
        let pattern = parse_pattern_str("(a)-[:KNOWS]->(b)-[:KNOWS]->(c)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(chain.elements.len(), 5);
        // a, KNOWS->, b, KNOWS->, c
        assert!(matches!(&chain.elements[0], PatternElement::Node(_)));
        assert!(matches!(
            &chain.elements[1],
            PatternElement::Relationship(_)
        ));
        assert!(matches!(&chain.elements[2], PatternElement::Node(_)));
        assert!(matches!(
            &chain.elements[3],
            PatternElement::Relationship(_)
        ));
        assert!(matches!(&chain.elements[4], PatternElement::Node(_)));
    }

    // Multiple chains: (a)-[:KNOWS]->(b), (c)-[:LIKES]->(d)
    #[test]
    fn pattern_multiple_chains() {
        let pattern =
            parse_pattern_str("(a)-[:KNOWS]->(b), (c)-[:LIKES]->(d)").expect("should parse");
        assert_eq!(pattern.chains.len(), 2);
        assert_eq!(pattern.chains[0].elements.len(), 3);
        assert_eq!(pattern.chains[1].elements.len(), 3);
    }

    // -- TASK-102: Variable-length path parsing --

    // [*] -> min=1, max=None (unbounded)
    #[test]
    fn pattern_var_length_star_only() {
        let pattern = parse_pattern_str("(a)-[*]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.min_hops, Some(1));
            assert_eq!(rel.max_hops, None);
        } else {
            panic!("expected relationship");
        }
    }

    // [*N] -> min=N, max=N (exact hop)
    #[test]
    fn pattern_var_length_exact_hop() {
        let pattern = parse_pattern_str("(a)-[*3]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.min_hops, Some(3));
            assert_eq!(rel.max_hops, Some(3));
        } else {
            panic!("expected relationship");
        }
    }

    // [*N..M] -> min=N, max=M
    #[test]
    fn pattern_var_length_range() {
        let pattern = parse_pattern_str("(a)-[*1..3]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.min_hops, Some(1));
            assert_eq!(rel.max_hops, Some(3));
        } else {
            panic!("expected relationship");
        }
    }

    // [*N..] -> min=N, max=None
    #[test]
    fn pattern_var_length_open_end() {
        let pattern = parse_pattern_str("(a)-[*2..]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.min_hops, Some(2));
            assert_eq!(rel.max_hops, None);
        } else {
            panic!("expected relationship");
        }
    }

    // [*..M] -> min=1, max=M
    #[test]
    fn pattern_var_length_open_start() {
        let pattern = parse_pattern_str("(a)-[*..5]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.min_hops, Some(1));
            assert_eq!(rel.max_hops, Some(5));
        } else {
            panic!("expected relationship");
        }
    }

    // [:TYPE*N..M] -> typed + bounded
    #[test]
    fn pattern_var_length_typed_bounded() {
        let pattern = parse_pattern_str("(a)-[:KNOWS*2..4]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.rel_types, vec!["KNOWS".to_string()]);
            assert_eq!(rel.min_hops, Some(2));
            assert_eq!(rel.max_hops, Some(4));
        } else {
            panic!("expected relationship");
        }
    }

    // [r:TYPE*2..5] -> variable + typed + bounded
    #[test]
    fn pattern_var_length_variable_typed_bounded() {
        let pattern = parse_pattern_str("(a)-[r:KNOWS*2..5]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.variable, Some("r".to_string()));
            assert_eq!(rel.rel_types, vec!["KNOWS".to_string()]);
            assert_eq!(rel.min_hops, Some(2));
            assert_eq!(rel.max_hops, Some(5));
        } else {
            panic!("expected relationship");
        }
    }

    // Incoming variable-length: (a)<-[*1..3]-(b)
    #[test]
    fn pattern_var_length_incoming() {
        let pattern = parse_pattern_str("(a)<-[*1..3]-(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.direction, RelDirection::Incoming);
            assert_eq!(rel.min_hops, Some(1));
            assert_eq!(rel.max_hops, Some(3));
        } else {
            panic!("expected relationship");
        }
    }

    // [*0..1] zero-length path
    #[test]
    fn pattern_var_length_zero() {
        let pattern = parse_pattern_str("(a)-[*0..1]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.min_hops, Some(0));
            assert_eq!(rel.max_hops, Some(1));
        } else {
            panic!("expected relationship");
        }
    }

    // Variable-length with variable only (no type): [r*2..5]
    #[test]
    fn pattern_var_length_variable_no_type() {
        let pattern = parse_pattern_str("(a)-[r*2..5]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.variable, Some("r".to_string()));
            assert_eq!(rel.rel_types, Vec::<String>::new());
            assert_eq!(rel.min_hops, Some(2));
            assert_eq!(rel.max_hops, Some(5));
        } else {
            panic!("expected relationship");
        }
    }

    // Regular rel (no *) still has None/None
    #[test]
    fn pattern_regular_rel_no_hops() {
        let pattern = parse_pattern_str("(a)-[:KNOWS]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        if let PatternElement::Relationship(rel) = &chain.elements[1] {
            assert_eq!(rel.min_hops, None);
            assert_eq!(rel.max_hops, None);
        } else {
            panic!("expected relationship");
        }
    }

    // Relationship with only variable, no type: (a)-[r]->(b)
    #[test]
    fn pattern_rel_variable_only() {
        let pattern = parse_pattern_str("(a)-[r]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: Some("r".to_string()),
                rel_types: vec![],
                direction: RelDirection::Outgoing,
                properties: None,
                min_hops: None,
                max_hops: None,
            })
        );
    }

    // Empty relationship brackets: (a)-[]->(b)
    #[test]
    fn pattern_empty_rel_brackets() {
        let pattern = parse_pattern_str("(a)-[]->(b)").expect("should parse");
        let chain = &pattern.chains[0];
        assert_eq!(
            chain.elements[1],
            PatternElement::Relationship(RelationshipPattern {
                variable: None,
                rel_types: vec![],
                direction: RelDirection::Outgoing,
                properties: None,
                min_hops: None,
                max_hops: None,
            })
        );
    }
}
