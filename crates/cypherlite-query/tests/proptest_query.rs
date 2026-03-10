// Property-based tests for cypherlite-query lexer and parser (TASK-058).
//
// These tests use proptest to fuzz the lexer and parser, verifying:
// 1. No panics on arbitrary input
// 2. Structural properties of valid queries
// 3. Token fidelity for literals

use cypherlite_query::lexer::{lex, Token};
use cypherlite_query::parser::parse_query;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// High case count for cheap operations (lexer, expression parsing).
fn fast_config() -> ProptestConfig {
    ProptestConfig {
        cases: 10_000,
        ..ProptestConfig::default()
    }
}

/// Lower case count for heavier operations (full parse).
fn slow_config() -> ProptestConfig {
    ProptestConfig {
        cases: 1_000,
        ..ProptestConfig::default()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// All Cypher keywords recognized by the CypherLite lexer (case-insensitive).
const CYPHER_KEYWORDS: &[&str] = &[
    "match", "return", "create", "as", "distinct", "true", "false", "null", "and", "or", "not",
    "is", "count", "where", "set", "remove", "delete", "detach", "optional", "merge", "with",
    "order", "by", "limit", "skip", "asc", "desc", "on",
];

/// Returns true if the given string is a Cypher keyword (case-insensitive).
fn is_keyword(s: &str) -> bool {
    CYPHER_KEYWORDS.contains(&s.to_lowercase().as_str())
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Strategy producing random ASCII/punctuation strings likely to exercise
/// lexer edge cases.
fn cypher_like_string() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9_ (){}\\[\\]:.,;=<>!+\\-/*%'\"\\n\\t@#$^&~]{0,200}")
        .expect("regex should compile")
}

/// Strategy producing a valid Cypher identifier (letter or underscore
/// followed by alphanumerics/underscores).
fn ident_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z_][a-zA-Z0-9_]{0,15}").expect("regex should compile")
}

/// Strategy producing a valid Cypher label name (starts with uppercase).
fn label_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Z][a-zA-Z0-9_]{0,15}").expect("regex should compile")
}

/// Strategy producing a valid single-quoted string literal body (no
/// unescaped single quotes or backslashes).
fn safe_string_body() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 _.,!?:;\\-]{0,50}").expect("regex should compile")
}

/// Strategy producing a random integer literal string.
fn integer_literal_strategy() -> impl Strategy<Value = i64> {
    0i64..=999_999
}

/// Strategy producing a random float literal string (digits.digits).
fn float_literal_strategy() -> impl Strategy<Value = (u32, u32)> {
    (0u32..=9999, 0u32..=9999)
}

/// Strategy producing an arithmetic operator.
fn arith_op() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("+"), Just("-"), Just("*"), Just("/"), Just("%"),]
}

// ---------------------------------------------------------------------------
// 1. Lexer robustness: arbitrary strings never panic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(fast_config())]

    /// Feeding arbitrary `String` values to the lexer must never panic.
    /// The result is always either Ok (tokens) or Err (LexError).
    #[test]
    fn lexer_never_panics_on_arbitrary_string(input in any::<String>()) {
        // We only care that this does not panic.
        let _ = lex(&input);
    }

    /// Feeding ASCII/punctuation-heavy strings to the lexer must never panic.
    #[test]
    fn lexer_never_panics_on_cypher_like_string(input in cypher_like_string()) {
        let _ = lex(&input);
    }

    /// Random bytes decoded as (possibly invalid) UTF-8 lossy -- lexer must
    /// not panic even on replaced-character output.
    #[test]
    fn lexer_never_panics_on_lossy_utf8(bytes in prop::collection::vec(any::<u8>(), 0..300)) {
        let input = String::from_utf8_lossy(&bytes).into_owned();
        let _ = lex(&input);
    }
}

// ---------------------------------------------------------------------------
// 2. Parser robustness: arbitrary strings never panic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(slow_config())]

    /// Feeding arbitrary `String` values to the parser must never panic.
    /// The result is always Ok(Query) or Err(ParseError).
    #[test]
    fn parser_never_panics_on_arbitrary_string(input in any::<String>()) {
        let _ = parse_query(&input);
    }

    /// Cypher-like random strings must not panic the parser.
    #[test]
    fn parser_never_panics_on_cypher_like_string(input in cypher_like_string()) {
        let _ = parse_query(&input);
    }

    /// Random bytes decoded as lossy UTF-8 must not panic the parser.
    #[test]
    fn parser_never_panics_on_lossy_utf8(bytes in prop::collection::vec(any::<u8>(), 0..300)) {
        let input = String::from_utf8_lossy(&bytes).into_owned();
        let _ = parse_query(&input);
    }
}

// ---------------------------------------------------------------------------
// 3. Roundtrip-like properties for valid MATCH queries
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(slow_config())]

    /// Randomly generated `MATCH (var:Label) RETURN var` queries must parse
    /// successfully.
    #[test]
    fn valid_match_return_parses(
        var in ident_strategy(),
        label in label_strategy(),
    ) {
        prop_assume!(!is_keyword(&var) && !is_keyword(&label));
        let query = format!("MATCH ({var}:{label}) RETURN {var}");
        let result = parse_query(&query);
        prop_assert!(
            result.is_ok(),
            "failed to parse valid MATCH/RETURN: {} -- error: {:?}",
            query,
            result.err()
        );
    }

    /// Randomly generated `MATCH (var:Label) WHERE var.prop > 0 RETURN var`
    /// queries must parse successfully.
    #[test]
    fn valid_match_where_return_parses(
        var in ident_strategy(),
        label in label_strategy(),
        prop in ident_strategy(),
        val in 0i64..=9999,
    ) {
        prop_assume!(!is_keyword(&var) && !is_keyword(&label) && !is_keyword(&prop));
        let query = format!(
            "MATCH ({var}:{label}) WHERE {var}.{prop} > {val} RETURN {var}"
        );
        let result = parse_query(&query);
        prop_assert!(
            result.is_ok(),
            "failed to parse valid MATCH/WHERE/RETURN: {} -- error: {:?}",
            query,
            result.err()
        );
    }

    /// MATCH with relationship pattern should parse.
    #[test]
    fn valid_match_relationship_parses(
        a in ident_strategy(),
        b in ident_strategy(),
        rel_type in label_strategy(),
    ) {
        prop_assume!(!is_keyword(&a) && !is_keyword(&b) && !is_keyword(&rel_type));
        let query = format!("MATCH ({a})-[:{rel_type}]->({b}) RETURN {a}, {b}");
        let result = parse_query(&query);
        prop_assert!(
            result.is_ok(),
            "failed to parse valid relationship MATCH: {} -- error: {:?}",
            query,
            result.err()
        );
    }
}

// ---------------------------------------------------------------------------
// 3b. Roundtrip-like properties for valid CREATE queries
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(slow_config())]

    /// CREATE (var:Label {key: 'value'}) must parse successfully.
    #[test]
    fn valid_create_with_string_property_parses(
        var in ident_strategy(),
        label in label_strategy(),
        key in ident_strategy(),
        val in safe_string_body(),
    ) {
        prop_assume!(!is_keyword(&var) && !is_keyword(&label) && !is_keyword(&key));
        let query = format!("CREATE ({var}:{label} {{{key}: '{val}'}})");
        let result = parse_query(&query);
        prop_assert!(
            result.is_ok(),
            "failed to parse valid CREATE: {} -- error: {:?}",
            query,
            result.err()
        );
    }

    /// CREATE (var:Label {key: integer}) must parse successfully.
    #[test]
    fn valid_create_with_integer_property_parses(
        var in ident_strategy(),
        label in label_strategy(),
        key in ident_strategy(),
        val in integer_literal_strategy(),
    ) {
        prop_assume!(!is_keyword(&var) && !is_keyword(&label) && !is_keyword(&key));
        let query = format!("CREATE ({var}:{label} {{{key}: {val}}})");
        let result = parse_query(&query);
        prop_assert!(
            result.is_ok(),
            "failed to parse valid CREATE: {} -- error: {:?}",
            query,
            result.err()
        );
    }

    /// CREATE with multiple properties must parse.
    #[test]
    fn valid_create_multi_property_parses(
        var in ident_strategy(),
        label in label_strategy(),
        k1 in ident_strategy(),
        v1 in integer_literal_strategy(),
        k2 in ident_strategy(),
        v2 in safe_string_body(),
    ) {
        prop_assume!(
            !is_keyword(&var) && !is_keyword(&label)
            && !is_keyword(&k1) && !is_keyword(&k2)
        );
        let query = format!(
            "CREATE ({var}:{label} {{{k1}: {v1}, {k2}: '{v2}'}})"
        );
        let result = parse_query(&query);
        prop_assert!(
            result.is_ok(),
            "failed to parse valid multi-prop CREATE: {} -- error: {:?}",
            query,
            result.err()
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Expression parser: arithmetic expressions never panic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(fast_config())]

    /// Random arithmetic expression of the form `RETURN a + b * c - d` with
    /// integer and float literals must not panic the parser.
    #[test]
    fn arithmetic_expression_never_panics(
        a in integer_literal_strategy(),
        op1 in arith_op(),
        b in integer_literal_strategy(),
        op2 in arith_op(),
        c in integer_literal_strategy(),
    ) {
        let query = format!("RETURN {a} {op1} {b} {op2} {c}");
        let result = parse_query(&query);
        // All of these are syntactically valid RETURN expressions.
        prop_assert!(
            result.is_ok(),
            "failed to parse arithmetic expression: {} -- error: {:?}",
            query,
            result.err()
        );
    }

    /// Float arithmetic expressions must also parse without panic.
    #[test]
    fn float_arithmetic_expression_never_panics(
        (a_int, a_frac) in float_literal_strategy(),
        op1 in arith_op(),
        (b_int, b_frac) in float_literal_strategy(),
        op2 in arith_op(),
        c in integer_literal_strategy(),
    ) {
        let query = format!(
            "RETURN {a_int}.{a_frac} {op1} {b_int}.{b_frac} {op2} {c}"
        );
        // We only care that this does not panic. Some combinations may fail
        // to parse (e.g., leading zeros in floats) but must not panic.
        let _ = parse_query(&query);
    }

    /// Deeply nested parenthesized arithmetic should not panic.
    #[test]
    fn nested_arithmetic_parens_never_panic(
        depth in 1usize..=20,
        val in integer_literal_strategy(),
    ) {
        let open_parens = "(".repeat(depth);
        let close_parens = ")".repeat(depth);
        let query = format!("RETURN {open_parens}{val}{close_parens}");
        let _ = parse_query(&query);
    }
}

// ---------------------------------------------------------------------------
// 5. Lexer token fidelity: literals preserve their values
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(fast_config())]

    /// Integer literal tokens must preserve the parsed numeric value.
    #[test]
    fn lexer_preserves_integer_value(val in 0i64..=999_999_999) {
        let input = val.to_string();
        let tokens = lex(&input).expect("integer literal should lex");
        prop_assert_eq!(tokens.len(), 1, "expected exactly one token");
        match &tokens[0].0 {
            Token::Integer(v) => prop_assert_eq!(*v, val),
            other => prop_assert!(false, "expected Integer token, got {:?}", other),
        }
    }

    /// Float literal tokens must preserve the parsed numeric value.
    #[test]
    fn lexer_preserves_float_value(
        int_part in 0u32..=99999,
        frac_part in 0u32..=99999,
    ) {
        let input = format!("{int_part}.{frac_part}");
        let expected: f64 = input.parse().expect("should parse as f64");
        let tokens = lex(&input).expect("float literal should lex");
        prop_assert_eq!(tokens.len(), 1, "expected exactly one token");
        match &tokens[0].0 {
            Token::Float(v) => {
                prop_assert!(
                    (*v - expected).abs() < f64::EPSILON,
                    "float mismatch: got {} expected {}",
                    v,
                    expected
                );
            }
            other => prop_assert!(false, "expected Float token, got {:?}", other),
        }
    }

    /// String literal tokens must preserve their content through lex
    /// round-trip (for safe ASCII content without escapes).
    #[test]
    fn lexer_preserves_string_content(body in safe_string_body()) {
        let input = format!("'{body}'");
        let tokens = lex(&input).expect("string literal should lex");
        prop_assert_eq!(tokens.len(), 1, "expected exactly one token");
        match &tokens[0].0 {
            Token::StringLiteral(s) => prop_assert_eq!(s, &body),
            other => prop_assert!(false, "expected StringLiteral, got {:?}", other),
        }
    }

    /// Identifiers must preserve their text content.
    #[test]
    fn lexer_preserves_identifier(name in ident_strategy()) {
        // Skip identifiers that collide with keywords.
        prop_assume!(!is_keyword(&name));

        let tokens = lex(&name).expect("identifier should lex");
        prop_assert_eq!(tokens.len(), 1, "expected exactly one token");
        match &tokens[0].0 {
            Token::Ident(s) => prop_assert_eq!(s, &name),
            other => prop_assert!(false, "expected Ident token, got {:?}", other),
        }
    }

    /// Lexer span end minus start should equal the byte length of the
    /// source slice for integer tokens.
    #[test]
    fn lexer_span_matches_source_length_for_integers(val in 0i64..=999_999_999) {
        let input = val.to_string();
        let tokens = lex(&input).expect("integer literal should lex");
        prop_assert_eq!(tokens.len(), 1);
        let span = &tokens[0].1;
        prop_assert_eq!(span.end - span.start, input.len());
    }
}
