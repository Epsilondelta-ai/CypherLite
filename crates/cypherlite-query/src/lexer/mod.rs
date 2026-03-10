// Lexer module: logos-based tokenizer for openCypher subset

use logos::Logos;
use std::fmt;

// ---------------------------------------------------------------------------
// Span
// ---------------------------------------------------------------------------

/// Byte-offset span in the source input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

// ---------------------------------------------------------------------------
// LexError
// ---------------------------------------------------------------------------

/// Error produced when the lexer encounters an unrecognized character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    /// Byte offset of the unrecognized character.
    pub position: usize,
    /// The unrecognized character (if available).
    pub character: Option<char>,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.character {
            Some(ch) => write!(
                f,
                "unexpected character '{}' at byte offset {}",
                ch, self.position
            ),
            None => write!(f, "unexpected token at byte offset {}", self.position),
        }
    }
}

impl std::error::Error for LexError {}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/// Lexical token for the openCypher subset supported by CypherLite.
#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\r\n]+")]
pub enum Token {
    // -- P0 Keywords (case-insensitive) ------------------------------------
    #[regex("(?i)match", priority = 10)]
    Match,
    #[regex("(?i)return", priority = 10)]
    Return,
    #[regex("(?i)create", priority = 10)]
    Create,
    #[regex("(?i)as", priority = 10)]
    As,
    #[regex("(?i)distinct", priority = 10)]
    Distinct,
    #[regex("(?i)true", priority = 10)]
    True,
    #[regex("(?i)false", priority = 10)]
    False,
    #[regex("(?i)null", priority = 10)]
    Null,
    #[regex("(?i)and", priority = 10)]
    And,
    #[regex("(?i)or", priority = 10)]
    Or,
    #[regex("(?i)not", priority = 10)]
    Not,
    #[regex("(?i)is", priority = 10)]
    Is,
    #[regex("(?i)count", priority = 10)]
    Count,

    // -- P1 Keywords -------------------------------------------------------
    #[regex("(?i)where", priority = 10)]
    Where,
    #[regex("(?i)set", priority = 10)]
    Set,
    #[regex("(?i)remove", priority = 10)]
    Remove,
    #[regex("(?i)delete", priority = 10)]
    Delete,
    #[regex("(?i)detach", priority = 10)]
    Detach,
    #[regex("(?i)optional", priority = 10)]
    Optional,

    // -- P2 Keywords -------------------------------------------------------
    #[regex("(?i)merge", priority = 10)]
    Merge,
    #[regex("(?i)with", priority = 10)]
    With,
    #[regex("(?i)order", priority = 10)]
    Order,
    #[regex("(?i)by", priority = 10)]
    By,
    #[regex("(?i)limit", priority = 10)]
    Limit,
    #[regex("(?i)skip", priority = 10)]
    Skip,
    #[regex("(?i)asc", priority = 10)]
    Asc,
    #[regex("(?i)desc", priority = 10)]
    Desc,
    #[regex("(?i)on", priority = 10)]
    On,

    // -- Literals ----------------------------------------------------------
    /// Floating-point literal (must come before integer to match greedily).
    #[regex(r"[0-9]+\.[0-9]+", lex_float, priority = 3)]
    Float(f64),

    /// Integer literal.
    #[regex(r"[0-9]+", lex_integer, priority = 2)]
    Integer(i64),

    /// Single-quoted string literal with escape sequences.
    #[regex(r"'([^'\\]|\\.)*'", lex_string)]
    StringLiteral(String),

    // -- Identifiers -------------------------------------------------------
    /// Backtick-quoted identifier.
    #[regex(r"`[^`]+`", lex_backtick_ident)]
    BacktickIdent(String),

    /// Regular identifier (lower priority than keywords).
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", lex_ident, priority = 1)]
    Ident(String),

    /// Parameter reference ($name).
    #[regex(r"\$[a-zA-Z_][a-zA-Z0-9_]*", lex_param)]
    Parameter(String),

    // -- Operators ---------------------------------------------------------
    #[token("<>")]
    NotEqual,
    #[token("!=")]
    BangEqual,
    #[token("<=")]
    LessEqual,
    #[token(">=")]
    GreaterEqual,
    #[token("=")]
    Eq,
    #[token("<")]
    Less,
    #[token(">")]
    Greater,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    // -- Arrow tokens ------------------------------------------------------
    #[token("->")]
    ArrowRight,
    #[token("<-")]
    ArrowLeft,
    #[token("--")]
    DoubleDash,

    // -- Punctuation -------------------------------------------------------
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,
    #[token(",")]
    Comma,
    #[token("|")]
    Pipe,
}

// ---------------------------------------------------------------------------
// Callback helpers
// ---------------------------------------------------------------------------

fn lex_float(lex: &mut logos::Lexer<Token>) -> Option<f64> {
    lex.slice().parse::<f64>().ok()
}

fn lex_integer(lex: &mut logos::Lexer<Token>) -> Option<i64> {
    lex.slice().parse::<i64>().ok()
}

fn lex_ident(lex: &mut logos::Lexer<Token>) -> String {
    lex.slice().to_string()
}

fn lex_backtick_ident(lex: &mut logos::Lexer<Token>) -> String {
    let s = lex.slice();
    // Strip surrounding backticks
    s[1..s.len() - 1].to_string()
}

fn lex_param(lex: &mut logos::Lexer<Token>) -> String {
    // Strip leading '$'
    lex.slice()[1..].to_string()
}

/// Process a single-quoted string literal, resolving escape sequences.
fn lex_string(lex: &mut logos::Lexer<Token>) -> String {
    let raw = lex.slice();
    // Strip surrounding quotes
    let inner = &raw[1..raw.len() - 1];
    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('\'') => result.push('\''),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Public lex function
// ---------------------------------------------------------------------------

/// Tokenize the input string, returning a vector of (Token, Span) pairs.
///
/// Returns `Err(LexError)` on the first unrecognized character.
pub fn lex(input: &str) -> Result<Vec<(Token, Span)>, LexError> {
    let mut lexer = Token::lexer(input);
    let mut tokens = Vec::new();

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        match result {
            Ok(token) => {
                tokens.push((
                    token,
                    Span {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
            Err(()) => {
                let position = span.start;
                let character = input[position..].chars().next();
                return Err(LexError {
                    position,
                    character,
                });
            }
        }
    }

    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to extract just the tokens (without spans) for easier assertions.
    fn tokens(input: &str) -> Vec<Token> {
        lex(input)
            .expect("lexing should succeed")
            .into_iter()
            .map(|(tok, _)| tok)
            .collect()
    }

    // ---- LEX-T001: Empty input ------------------------------------------

    #[test]
    fn lex_t001_empty_input() {
        let result = lex("");
        assert_eq!(result.expect("should succeed"), vec![]);
    }

    #[test]
    fn lex_t001_whitespace_only() {
        let result = lex("   \t\n\r  ");
        assert_eq!(result.expect("should succeed"), vec![]);
    }

    // ---- LEX-T002: MATCH (n) RETURN n -----------------------------------

    #[test]
    fn lex_t002_match_return_basic() {
        let toks = tokens("MATCH (n) RETURN n");
        assert_eq!(
            toks,
            vec![
                Token::Match,
                Token::LParen,
                Token::Ident("n".to_string()),
                Token::RParen,
                Token::Return,
                Token::Ident("n".to_string()),
            ]
        );
    }

    // ---- LEX-T003: Case-insensitive keywords ----------------------------

    #[test]
    fn lex_t003_case_insensitive_match() {
        assert_eq!(tokens("mAtCh"), vec![Token::Match]);
    }

    #[test]
    fn lex_t003_case_insensitive_return() {
        assert_eq!(tokens("ReTuRn"), vec![Token::Return]);
    }

    #[test]
    fn lex_t003_case_insensitive_create() {
        assert_eq!(tokens("cReAtE"), vec![Token::Create]);
    }

    #[test]
    fn lex_t003_all_keywords_lowercase() {
        let kw_pairs = vec![
            ("match", Token::Match),
            ("return", Token::Return),
            ("create", Token::Create),
            ("as", Token::As),
            ("distinct", Token::Distinct),
            ("true", Token::True),
            ("false", Token::False),
            ("null", Token::Null),
            ("and", Token::And),
            ("or", Token::Or),
            ("not", Token::Not),
            ("is", Token::Is),
            ("count", Token::Count),
            ("where", Token::Where),
            ("set", Token::Set),
            ("remove", Token::Remove),
            ("delete", Token::Delete),
            ("detach", Token::Detach),
            ("optional", Token::Optional),
            ("merge", Token::Merge),
            ("with", Token::With),
            ("order", Token::Order),
            ("by", Token::By),
            ("limit", Token::Limit),
            ("skip", Token::Skip),
            ("asc", Token::Asc),
            ("desc", Token::Desc),
            ("on", Token::On),
        ];
        for (input, expected) in kw_pairs {
            assert_eq!(
                tokens(input),
                vec![expected],
                "keyword '{}' should be recognized",
                input,
            );
        }
    }

    // ---- LEX-T004: String literal with escape sequences -----------------

    #[test]
    fn lex_t004_string_with_newline_escape() {
        let toks = tokens(r"'hello\nworld'");
        assert_eq!(toks, vec![Token::StringLiteral("hello\nworld".to_string())]);
    }

    #[test]
    fn lex_t004_string_with_tab_escape() {
        let toks = tokens(r"'a\tb'");
        assert_eq!(toks, vec![Token::StringLiteral("a\tb".to_string())]);
    }

    #[test]
    fn lex_t004_string_with_backslash_escape() {
        let toks = tokens(r"'a\\b'");
        assert_eq!(toks, vec![Token::StringLiteral("a\\b".to_string())]);
    }

    #[test]
    fn lex_t004_string_with_quote_escape() {
        let toks = tokens(r"'it\'s'");
        assert_eq!(toks, vec![Token::StringLiteral("it's".to_string())]);
    }

    #[test]
    fn lex_t004_empty_string() {
        let toks = tokens("''");
        assert_eq!(toks, vec![Token::StringLiteral(String::new())]);
    }

    // ---- LEX-T005: Unrecognized character -> LexError -------------------

    #[test]
    fn lex_t005_unrecognized_at_sign() {
        let result = lex("@");
        let err = result.expect_err("should fail on '@'");
        assert_eq!(err.position, 0);
        assert_eq!(err.character, Some('@'));
    }

    #[test]
    fn lex_t005_unrecognized_after_valid_tokens() {
        let result = lex("MATCH @");
        let err = result.expect_err("should fail on '@'");
        assert_eq!(err.position, 6);
        assert_eq!(err.character, Some('@'));
    }

    // ---- LEX-T006: Integer and float literals ---------------------------

    #[test]
    fn lex_t006_integer_42() {
        let toks = tokens("42");
        assert_eq!(toks, vec![Token::Integer(42)]);
    }

    #[test]
    fn lex_t006_float_3_14() {
        let toks = tokens("3.15");
        assert_eq!(toks, vec![Token::Float(3.15)]);
    }

    #[test]
    fn lex_t006_integer_zero() {
        let toks = tokens("0");
        assert_eq!(toks, vec![Token::Integer(0)]);
    }

    #[test]
    fn lex_t006_float_zero_point_zero() {
        let toks = tokens("0.0");
        assert_eq!(toks, vec![Token::Float(0.0)]);
    }

    // ---- LEX-T007: Unicode in backtick-quoted identifiers ---------------

    #[test]
    fn lex_t007_backtick_unicode_identifier() {
        let toks = tokens("`user name`");
        assert_eq!(toks, vec![Token::BacktickIdent("user name".to_string())]);
    }

    #[test]
    fn lex_t007_backtick_korean_identifier() {
        let toks = tokens("`이름`");
        assert_eq!(toks, vec![Token::BacktickIdent("이름".to_string())]);
    }

    // ---- Operators ------------------------------------------------------

    #[test]
    fn lex_operators_comparison() {
        let toks = tokens("= <> != < <= > >=");
        assert_eq!(
            toks,
            vec![
                Token::Eq,
                Token::NotEqual,
                Token::BangEqual,
                Token::Less,
                Token::LessEqual,
                Token::Greater,
                Token::GreaterEqual,
            ]
        );
    }

    #[test]
    fn lex_operators_arithmetic() {
        let toks = tokens("+ - * / %");
        assert_eq!(
            toks,
            vec![
                Token::Plus,
                Token::Minus,
                Token::Star,
                Token::Slash,
                Token::Percent,
            ]
        );
    }

    // ---- Punctuation ----------------------------------------------------

    #[test]
    fn lex_punctuation() {
        let toks = tokens("( ) [ ] { } : . , |");
        assert_eq!(
            toks,
            vec![
                Token::LParen,
                Token::RParen,
                Token::LBracket,
                Token::RBracket,
                Token::LBrace,
                Token::RBrace,
                Token::Colon,
                Token::Dot,
                Token::Comma,
                Token::Pipe,
            ]
        );
    }

    // ---- Arrow tokens ---------------------------------------------------

    #[test]
    fn lex_arrow_right() {
        let toks = tokens("->");
        assert_eq!(toks, vec![Token::ArrowRight]);
    }

    #[test]
    fn lex_arrow_left() {
        let toks = tokens("<-");
        assert_eq!(toks, vec![Token::ArrowLeft]);
    }

    #[test]
    fn lex_double_dash() {
        let toks = tokens("--");
        assert_eq!(toks, vec![Token::DoubleDash]);
    }

    // ---- Parameter tokens -----------------------------------------------

    #[test]
    fn lex_parameter() {
        let toks = tokens("$name");
        assert_eq!(toks, vec![Token::Parameter("name".to_string())]);
    }

    #[test]
    fn lex_parameter_with_underscore() {
        let toks = tokens("$user_id");
        assert_eq!(toks, vec![Token::Parameter("user_id".to_string())]);
    }

    // ---- Identifier vs keyword boundary ---------------------------------

    #[test]
    fn lex_identifier_starting_with_keyword_prefix() {
        // "matching" should be an identifier, not MATCH + "ing"
        let toks = tokens("matching");
        assert_eq!(toks, vec![Token::Ident("matching".to_string())]);
    }

    #[test]
    fn lex_identifier_returns() {
        // "returns" should be an identifier, not RETURN + "s"
        let toks = tokens("returns");
        assert_eq!(toks, vec![Token::Ident("returns".to_string())]);
    }

    #[test]
    fn lex_identifier_underscore_prefix() {
        let toks = tokens("_private");
        assert_eq!(toks, vec![Token::Ident("_private".to_string())]);
    }

    // ---- Span tracking --------------------------------------------------

    #[test]
    fn lex_span_tracking() {
        let result = lex("MATCH (n)").expect("should succeed");
        assert_eq!(result[0].1, Span { start: 0, end: 5 }); // MATCH
        assert_eq!(result[1].1, Span { start: 6, end: 7 }); // (
        assert_eq!(result[2].1, Span { start: 7, end: 8 }); // n
        assert_eq!(result[3].1, Span { start: 8, end: 9 }); // )
    }

    // ---- Complex queries ------------------------------------------------

    #[test]
    fn lex_relationship_pattern() {
        let toks = tokens("(a)-[:KNOWS]->(b)");
        assert_eq!(
            toks,
            vec![
                Token::LParen,
                Token::Ident("a".to_string()),
                Token::RParen,
                Token::Minus,
                Token::LBracket,
                Token::Colon,
                Token::Ident("KNOWS".to_string()),
                Token::RBracket,
                Token::ArrowRight,
                Token::LParen,
                Token::Ident("b".to_string()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn lex_where_clause() {
        let toks = tokens("WHERE n.age >= 18 AND n.name <> 'unknown'");
        assert_eq!(
            toks,
            vec![
                Token::Where,
                Token::Ident("n".to_string()),
                Token::Dot,
                Token::Ident("age".to_string()),
                Token::GreaterEqual,
                Token::Integer(18),
                Token::And,
                Token::Ident("n".to_string()),
                Token::Dot,
                Token::Ident("name".to_string()),
                Token::NotEqual,
                Token::StringLiteral("unknown".to_string()),
            ]
        );
    }

    #[test]
    fn lex_return_with_alias() {
        let toks = tokens("RETURN n.name AS username, COUNT(DISTINCT n)");
        assert_eq!(
            toks,
            vec![
                Token::Return,
                Token::Ident("n".to_string()),
                Token::Dot,
                Token::Ident("name".to_string()),
                Token::As,
                Token::Ident("username".to_string()),
                Token::Comma,
                Token::Count,
                Token::LParen,
                Token::Distinct,
                Token::Ident("n".to_string()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn lex_order_by_limit_skip() {
        let toks = tokens("ORDER BY n.age DESC SKIP 10 LIMIT 5");
        assert_eq!(
            toks,
            vec![
                Token::Order,
                Token::By,
                Token::Ident("n".to_string()),
                Token::Dot,
                Token::Ident("age".to_string()),
                Token::Desc,
                Token::Skip,
                Token::Integer(10),
                Token::Limit,
                Token::Integer(5),
            ]
        );
    }

    #[test]
    fn lex_create_with_properties() {
        let toks = tokens("CREATE (n:Person {name: 'Alice', age: 30})");
        assert_eq!(
            toks,
            vec![
                Token::Create,
                Token::LParen,
                Token::Ident("n".to_string()),
                Token::Colon,
                Token::Ident("Person".to_string()),
                Token::LBrace,
                Token::Ident("name".to_string()),
                Token::Colon,
                Token::StringLiteral("Alice".to_string()),
                Token::Comma,
                Token::Ident("age".to_string()),
                Token::Colon,
                Token::Integer(30),
                Token::RBrace,
                Token::RParen,
            ]
        );
    }

    #[test]
    fn lex_error_display() {
        let err = LexError {
            position: 5,
            character: Some('@'),
        };
        assert_eq!(err.to_string(), "unexpected character '@' at byte offset 5");
    }

    #[test]
    fn lex_error_display_no_char() {
        let err = LexError {
            position: 5,
            character: None,
        };
        assert_eq!(err.to_string(), "unexpected token at byte offset 5");
    }

    #[test]
    fn lex_boolean_literals_in_context() {
        let toks = tokens("true AND false OR NOT null IS null");
        assert_eq!(
            toks,
            vec![
                Token::True,
                Token::And,
                Token::False,
                Token::Or,
                Token::Not,
                Token::Null,
                Token::Is,
                Token::Null,
            ]
        );
    }

    #[test]
    fn lex_optional_match() {
        let toks = tokens("OPTIONAL MATCH");
        assert_eq!(toks, vec![Token::Optional, Token::Match]);
    }

    #[test]
    fn lex_detach_delete() {
        let toks = tokens("DETACH DELETE n");
        assert_eq!(
            toks,
            vec![Token::Detach, Token::Delete, Token::Ident("n".to_string()),]
        );
    }

    #[test]
    fn lex_merge_on() {
        let toks = tokens("MERGE (n) ON MATCH SET n.x = 1");
        assert_eq!(
            toks,
            vec![
                Token::Merge,
                Token::LParen,
                Token::Ident("n".to_string()),
                Token::RParen,
                Token::On,
                Token::Match,
                Token::Set,
                Token::Ident("n".to_string()),
                Token::Dot,
                Token::Ident("x".to_string()),
                Token::Eq,
                Token::Integer(1),
            ]
        );
    }

    #[test]
    fn lex_with_clause() {
        let toks = tokens("WITH n, m");
        assert_eq!(
            toks,
            vec![
                Token::With,
                Token::Ident("n".to_string()),
                Token::Comma,
                Token::Ident("m".to_string()),
            ]
        );
    }

    #[test]
    fn lex_remove_keyword() {
        let toks = tokens("REMOVE n.prop");
        assert_eq!(
            toks,
            vec![
                Token::Remove,
                Token::Ident("n".to_string()),
                Token::Dot,
                Token::Ident("prop".to_string()),
            ]
        );
    }

    #[test]
    fn lex_left_arrow_relationship() {
        let toks = tokens("(a)<-[:KNOWS]-(b)");
        assert_eq!(
            toks,
            vec![
                Token::LParen,
                Token::Ident("a".to_string()),
                Token::RParen,
                Token::ArrowLeft,
                Token::LBracket,
                Token::Colon,
                Token::Ident("KNOWS".to_string()),
                Token::RBracket,
                Token::Minus,
                Token::LParen,
                Token::Ident("b".to_string()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn lex_undirected_relationship() {
        let toks = tokens("(a)--(b)");
        assert_eq!(
            toks,
            vec![
                Token::LParen,
                Token::Ident("a".to_string()),
                Token::RParen,
                Token::DoubleDash,
                Token::LParen,
                Token::Ident("b".to_string()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn lex_arithmetic_expression() {
        let toks = tokens("1 + 2 * 3 - 4 / 5 % 6");
        assert_eq!(
            toks,
            vec![
                Token::Integer(1),
                Token::Plus,
                Token::Integer(2),
                Token::Star,
                Token::Integer(3),
                Token::Minus,
                Token::Integer(4),
                Token::Slash,
                Token::Integer(5),
                Token::Percent,
                Token::Integer(6),
            ]
        );
    }
}
