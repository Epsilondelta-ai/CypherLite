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
        })
    }

    /// Parse content inside relationship brackets: `[variable :TYPE | TYPE2 {props}]`
    fn parse_relationship_content(&mut self) -> Result<RelContentResult, ParseError> {
        let mut variable = None;
        let mut rel_types = Vec::new();
        let mut properties = None;

        // Check for variable-length path star
        if self.check(&Token::Star) {
            return Err(self.error("variable-length paths (*) are not supported"));
        }

        // Optional variable
        if let Some(Token::Ident(_) | Token::BacktickIdent(_)) = self.peek() {
            // Only treat as variable if next is not just `]`
            // Could be a variable followed by colon (for type), or just a variable
            variable = Some(self.expect_ident()?);
        }

        // Check for variable-length path star after variable
        if self.check(&Token::Star) {
            return Err(self.error("variable-length paths (*) are not supported"));
        }

        // Optional relationship types: `:TYPE` or `:TYPE|TYPE2`
        if self.eat(&Token::Colon) {
            rel_types.push(self.expect_ident()?);
            while self.eat(&Token::Pipe) {
                rel_types.push(self.expect_ident()?);
            }
        }

        // Check for variable-length path star after types
        if self.check(&Token::Star) {
            return Err(self.error("variable-length paths (*) are not supported"));
        }

        // Optional properties
        if self.check(&Token::LBrace) {
            properties = Some(self.parse_map_literal()?);
        }

        Ok(RelContentResult {
            variable,
            rel_types,
            properties,
        })
    }
}

/// Intermediate result for relationship bracket content.
struct RelContentResult {
    variable: Option<String>,
    rel_types: Vec<String>,
    properties: Option<MapLiteral>,
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

    // Variable-length path -> UnsupportedSyntax error
    #[test]
    fn pattern_variable_length_error() {
        let result = parse_pattern_str("(a)-[*1..3]->(b)");
        assert!(result.is_err());
        let err = result.expect_err("should fail");
        assert!(
            err.message.contains("not supported"),
            "error message should mention unsupported: {}",
            err.message
        );
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
            })
        );
    }
}
