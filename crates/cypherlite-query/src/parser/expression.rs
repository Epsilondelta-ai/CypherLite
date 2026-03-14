// Pratt parser for expressions (arithmetic, comparison, boolean)

use super::ast::*;
use super::{ParseError, Parser};
use crate::lexer::Token;

/// Binding power for Pratt parser precedence.
/// Returns (left_bp, right_bp) for infix operators.
fn infix_binding_power(op: &BinaryOp) -> (u8, u8) {
    match op {
        BinaryOp::Or => (1, 2),
        BinaryOp::And => (3, 4),
        // Comparison operators are non-associative; use equal left/right
        BinaryOp::Eq
        | BinaryOp::Neq
        | BinaryOp::Lt
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Gte => (5, 6),
        BinaryOp::Add | BinaryOp::Sub => (7, 8),
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => (9, 10),
    }
}

/// Binding power for prefix unary operators.
/// Returns right_bp.
fn prefix_binding_power(op: &UnaryOp) -> u8 {
    match op {
        // NOT has same precedence as comparison level (between AND and comparison)
        UnaryOp::Not => 4,
        // Unary minus is high precedence
        UnaryOp::Neg => 11,
    }
}

/// Map a token to a binary operator, if applicable.
fn token_to_binary_op(token: &Token) -> Option<BinaryOp> {
    match token {
        Token::Plus => Some(BinaryOp::Add),
        Token::Minus => Some(BinaryOp::Sub),
        Token::Star => Some(BinaryOp::Mul),
        Token::Slash => Some(BinaryOp::Div),
        Token::Percent => Some(BinaryOp::Mod),
        Token::Eq => Some(BinaryOp::Eq),
        Token::NotEqual | Token::BangEqual => Some(BinaryOp::Neq),
        Token::Less => Some(BinaryOp::Lt),
        Token::LessEqual => Some(BinaryOp::Lte),
        Token::Greater => Some(BinaryOp::Gt),
        Token::GreaterEqual => Some(BinaryOp::Gte),
        Token::And => Some(BinaryOp::And),
        Token::Or => Some(BinaryOp::Or),
        _ => None,
    }
}

impl<'a> Parser<'a> {
    /// Parse an expression using Pratt parsing (precedence climbing).
    pub fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_expr_bp(0)
    }

    /// Parse an expression but stop before AND/OR (for BETWEEN TIME <expr> AND <expr>).
    /// This prevents the AND keyword from being consumed as a binary AND operator.
    pub fn parse_expression_no_and(&mut self) -> Result<Expression, ParseError> {
        // AND has left bp = 3, so using min_bp = 4 will stop before AND
        self.parse_expr_bp(4)
    }

    /// Pratt parser core: parse expression with minimum binding power.
    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expression, ParseError> {
        // Parse prefix / primary
        let mut lhs = self.parse_prefix()?;

        loop {
            // Property access (highest precedence postfix)
            if self.check(&Token::Dot) {
                self.advance();
                let prop = self.expect_ident()?;
                lhs = Expression::Property(Box::new(lhs), prop);
                continue;
            }

            // IS [NOT] NULL (postfix)
            if self.check(&Token::Is) {
                let (line, col) = self.offset_to_line_col(self.current_span().start);
                // IS NULL/IS NOT NULL has comparison-level precedence
                let is_bp: u8 = 5;
                if is_bp < min_bp {
                    break;
                }
                self.advance(); // consume IS
                let negated = self.eat(&Token::Not);
                if !self.check(&Token::Null) {
                    return Err(ParseError {
                        line,
                        column: col,
                        message: "expected NULL after IS [NOT]".to_string(),
                    });
                }
                self.advance(); // consume NULL
                lhs = Expression::IsNull(Box::new(lhs), negated);
                continue;
            }

            // Infix binary operator
            let op = match self.peek().and_then(token_to_binary_op) {
                Some(op) => op,
                None => break,
            };

            let (l_bp, r_bp) = infix_binding_power(&op);
            if l_bp < min_bp {
                break;
            }

            self.advance(); // consume operator token
            let rhs = self.parse_expr_bp(r_bp)?;
            lhs = Expression::BinaryOp(op, Box::new(lhs), Box::new(rhs));
        }

        Ok(lhs)
    }

    /// Parse a prefix expression (unary operators or primary).
    fn parse_prefix(&mut self) -> Result<Expression, ParseError> {
        match self.peek() {
            Some(Token::Not) => {
                self.advance();
                let r_bp = prefix_binding_power(&UnaryOp::Not);
                let expr = self.parse_expr_bp(r_bp)?;
                Ok(Expression::UnaryOp(UnaryOp::Not, Box::new(expr)))
            }
            Some(Token::Minus) => {
                self.advance();
                let r_bp = prefix_binding_power(&UnaryOp::Neg);
                let expr = self.parse_expr_bp(r_bp)?;
                Ok(Expression::UnaryOp(UnaryOp::Neg, Box::new(expr)))
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr_bp(0)?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(Token::LBracket) => self.parse_list_literal(),
            _ => self.parse_primary(),
        }
    }

    /// Parse a list literal: `[expr, expr, ...]`
    fn parse_list_literal(&mut self) -> Result<Expression, ParseError> {
        self.expect(&Token::LBracket)?;
        let mut elements = Vec::new();
        if !self.check(&Token::RBracket) {
            elements.push(self.parse_expression()?);
            while self.eat(&Token::Comma) {
                elements.push(self.parse_expression()?);
            }
        }
        self.expect(&Token::RBracket)?;
        Ok(Expression::ListLiteral(elements))
    }

    /// Parse a primary expression: literal, variable, function call, parameter.
    fn parse_primary(&mut self) -> Result<Expression, ParseError> {
        match self.peek().cloned() {
            Some(Token::Integer(n)) => {
                self.advance();
                Ok(Expression::Literal(Literal::Integer(n)))
            }
            Some(Token::Float(f)) => {
                self.advance();
                Ok(Expression::Literal(Literal::Float(f)))
            }
            Some(Token::StringLiteral(s)) => {
                self.advance();
                Ok(Expression::Literal(Literal::String(s)))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expression::Literal(Literal::Bool(true)))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expression::Literal(Literal::Bool(false)))
            }
            Some(Token::Null) => {
                self.advance();
                Ok(Expression::Literal(Literal::Null))
            }
            Some(Token::Parameter(name)) => {
                self.advance();
                Ok(Expression::Parameter(name))
            }
            // count(*) or count(DISTINCT x) or count(x)
            Some(Token::Count) => {
                self.advance();
                self.expect(&Token::LParen)?;
                if self.eat(&Token::Star) {
                    self.expect(&Token::RParen)?;
                    return Ok(Expression::CountStar);
                }
                let distinct = self.eat(&Token::Distinct);
                let arg = self.parse_expression()?;
                self.expect(&Token::RParen)?;
                Ok(Expression::FunctionCall {
                    name: "count".to_string(),
                    distinct,
                    args: vec![arg],
                })
            }
            // Generic function call: ident(args) or plain variable
            Some(Token::Ident(name)) => {
                self.advance();
                // Check for function call
                if self.check(&Token::LParen) {
                    self.advance(); // consume '('
                    let distinct = self.eat(&Token::Distinct);
                    let mut args = Vec::new();
                    if !self.check(&Token::RParen) {
                        args.push(self.parse_expression()?);
                        while self.eat(&Token::Comma) {
                            args.push(self.parse_expression()?);
                        }
                    }
                    self.expect(&Token::RParen)?;
                    Ok(Expression::FunctionCall {
                        name,
                        distinct,
                        args,
                    })
                } else {
                    Ok(Expression::Variable(name))
                }
            }
            Some(Token::BacktickIdent(name)) => {
                self.advance();
                Ok(Expression::Variable(name))
            }
            Some(tok) => Err(self.error(format!("unexpected token in expression: {:?}", tok))),
            None => Err(self.error("unexpected end of input in expression")),
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

    /// Helper: parse an expression from a string.
    fn parse_expr(input: &str) -> Result<Expression, ParseError> {
        let tokens = lex(input).expect("lexing should succeed");
        let mut parser = Parser::new(&tokens, input);
        parser.parse_expression()
    }

    // -- TASK-022: Expression parser tests --

    // Precedence: 1 + 2 * 3 -> Add(1, Mul(2, 3))
    #[test]
    fn expr_precedence_add_mul() {
        let expr = parse_expr("1 + 2 * 3").expect("should parse");
        assert_eq!(
            expr,
            Expression::BinaryOp(
                BinaryOp::Add,
                Box::new(Expression::Literal(Literal::Integer(1))),
                Box::new(Expression::BinaryOp(
                    BinaryOp::Mul,
                    Box::new(Expression::Literal(Literal::Integer(2))),
                    Box::new(Expression::Literal(Literal::Integer(3))),
                )),
            )
        );
    }

    // Parenthesized: (1 + 2) * 3 -> Mul(Add(1, 2), 3)
    #[test]
    fn expr_parenthesized() {
        let expr = parse_expr("(1 + 2) * 3").expect("should parse");
        assert_eq!(
            expr,
            Expression::BinaryOp(
                BinaryOp::Mul,
                Box::new(Expression::BinaryOp(
                    BinaryOp::Add,
                    Box::new(Expression::Literal(Literal::Integer(1))),
                    Box::new(Expression::Literal(Literal::Integer(2))),
                )),
                Box::new(Expression::Literal(Literal::Integer(3))),
            )
        );
    }

    // Comparison: n.age > 30
    #[test]
    fn expr_comparison_property() {
        let expr = parse_expr("n.age > 30").expect("should parse");
        assert_eq!(
            expr,
            Expression::BinaryOp(
                BinaryOp::Gt,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "age".to_string(),
                )),
                Box::new(Expression::Literal(Literal::Integer(30))),
            )
        );
    }

    // Boolean: a AND b OR c -> Or(And(a, b), c)
    #[test]
    fn expr_boolean_precedence() {
        let expr = parse_expr("a AND b OR c").expect("should parse");
        assert_eq!(
            expr,
            Expression::BinaryOp(
                BinaryOp::Or,
                Box::new(Expression::BinaryOp(
                    BinaryOp::And,
                    Box::new(Expression::Variable("a".to_string())),
                    Box::new(Expression::Variable("b".to_string())),
                )),
                Box::new(Expression::Variable("c".to_string())),
            )
        );
    }

    // NOT: NOT x
    #[test]
    fn expr_not_unary() {
        let expr = parse_expr("NOT x").expect("should parse");
        assert_eq!(
            expr,
            Expression::UnaryOp(
                UnaryOp::Not,
                Box::new(Expression::Variable("x".to_string())),
            )
        );
    }

    // NOT with AND: NOT a AND b -> And(Not(a), b)
    #[test]
    fn expr_not_and_precedence() {
        let expr = parse_expr("NOT a AND b").expect("should parse");
        assert_eq!(
            expr,
            Expression::BinaryOp(
                BinaryOp::And,
                Box::new(Expression::UnaryOp(
                    UnaryOp::Not,
                    Box::new(Expression::Variable("a".to_string())),
                )),
                Box::new(Expression::Variable("b".to_string())),
            )
        );
    }

    // Function call: count(n)
    #[test]
    fn expr_function_call_count() {
        let expr = parse_expr("count(n)").expect("should parse");
        assert_eq!(
            expr,
            Expression::FunctionCall {
                name: "count".to_string(),
                distinct: false,
                args: vec![Expression::Variable("n".to_string())],
            }
        );
    }

    // Function call: count(DISTINCT n)
    #[test]
    fn expr_function_call_count_distinct() {
        let expr = parse_expr("count(DISTINCT n)").expect("should parse");
        assert_eq!(
            expr,
            Expression::FunctionCall {
                name: "count".to_string(),
                distinct: true,
                args: vec![Expression::Variable("n".to_string())],
            }
        );
    }

    // count(*)
    #[test]
    fn expr_count_star() {
        let expr = parse_expr("count(*)").expect("should parse");
        assert_eq!(expr, Expression::CountStar);
    }

    // Generic function call: toUpper(n.name)
    #[test]
    fn expr_generic_function_call() {
        let expr = parse_expr("toUpper(n.name)").expect("should parse");
        assert_eq!(
            expr,
            Expression::FunctionCall {
                name: "toUpper".to_string(),
                distinct: false,
                args: vec![Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "name".to_string(),
                )],
            }
        );
    }

    // Multi-arg function: coalesce(a, b, c)
    #[test]
    fn expr_multi_arg_function() {
        let expr = parse_expr("coalesce(a, b, c)").expect("should parse");
        assert_eq!(
            expr,
            Expression::FunctionCall {
                name: "coalesce".to_string(),
                distinct: false,
                args: vec![
                    Expression::Variable("a".to_string()),
                    Expression::Variable("b".to_string()),
                    Expression::Variable("c".to_string()),
                ],
            }
        );
    }

    // Parameter: $name
    #[test]
    fn expr_parameter() {
        let expr = parse_expr("$name").expect("should parse");
        assert_eq!(expr, Expression::Parameter("name".to_string()));
    }

    // IS NULL
    #[test]
    fn expr_is_null() {
        let expr = parse_expr("n.name IS NULL").expect("should parse");
        assert_eq!(
            expr,
            Expression::IsNull(
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "name".to_string(),
                )),
                false,
            )
        );
    }

    // IS NOT NULL
    #[test]
    fn expr_is_not_null() {
        let expr = parse_expr("n.name IS NOT NULL").expect("should parse");
        assert_eq!(
            expr,
            Expression::IsNull(
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "name".to_string(),
                )),
                true,
            )
        );
    }

    // Literals
    #[test]
    fn expr_literal_integer() {
        let expr = parse_expr("42").expect("should parse");
        assert_eq!(expr, Expression::Literal(Literal::Integer(42)));
    }

    #[test]
    fn expr_literal_float() {
        let expr = parse_expr("3.15").expect("should parse");
        assert_eq!(expr, Expression::Literal(Literal::Float(3.15)));
    }

    #[test]
    fn expr_literal_string() {
        let expr = parse_expr("'hello'").expect("should parse");
        assert_eq!(
            expr,
            Expression::Literal(Literal::String("hello".to_string()))
        );
    }

    #[test]
    fn expr_literal_true() {
        let expr = parse_expr("true").expect("should parse");
        assert_eq!(expr, Expression::Literal(Literal::Bool(true)));
    }

    #[test]
    fn expr_literal_false() {
        let expr = parse_expr("false").expect("should parse");
        assert_eq!(expr, Expression::Literal(Literal::Bool(false)));
    }

    #[test]
    fn expr_literal_null() {
        let expr = parse_expr("null").expect("should parse");
        assert_eq!(expr, Expression::Literal(Literal::Null));
    }

    // Unary minus
    #[test]
    fn expr_unary_minus() {
        let expr = parse_expr("-42").expect("should parse");
        assert_eq!(
            expr,
            Expression::UnaryOp(
                UnaryOp::Neg,
                Box::new(Expression::Literal(Literal::Integer(42))),
            )
        );
    }

    // Chained property access: a.b.c
    #[test]
    fn expr_chained_property() {
        let expr = parse_expr("a.b.c").expect("should parse");
        assert_eq!(
            expr,
            Expression::Property(
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("a".to_string())),
                    "b".to_string(),
                )),
                "c".to_string(),
            )
        );
    }

    // Complex: n.age >= 18 AND n.name <> 'unknown'
    #[test]
    fn expr_complex_boolean_comparison() {
        let expr = parse_expr("n.age >= 18 AND n.name <> 'unknown'").expect("should parse");
        assert_eq!(
            expr,
            Expression::BinaryOp(
                BinaryOp::And,
                Box::new(Expression::BinaryOp(
                    BinaryOp::Gte,
                    Box::new(Expression::Property(
                        Box::new(Expression::Variable("n".to_string())),
                        "age".to_string(),
                    )),
                    Box::new(Expression::Literal(Literal::Integer(18))),
                )),
                Box::new(Expression::BinaryOp(
                    BinaryOp::Neq,
                    Box::new(Expression::Property(
                        Box::new(Expression::Variable("n".to_string())),
                        "name".to_string(),
                    )),
                    Box::new(Expression::Literal(Literal::String("unknown".to_string()))),
                )),
            )
        );
    }

    // != operator
    #[test]
    fn expr_bang_equal() {
        let expr = parse_expr("a != b").expect("should parse");
        assert_eq!(
            expr,
            Expression::BinaryOp(
                BinaryOp::Neq,
                Box::new(Expression::Variable("a".to_string())),
                Box::new(Expression::Variable("b".to_string())),
            )
        );
    }

    // Error case: empty input
    #[test]
    fn expr_error_empty() {
        let result = parse_expr("");
        assert!(result.is_err());
    }

    // Error case: unmatched paren
    #[test]
    fn expr_error_unmatched_paren() {
        let result = parse_expr("(1 + 2");
        assert!(result.is_err());
    }

    // ---- TASK-067/068: List literal expressions ----

    #[test]
    fn expr_list_literal_empty() {
        let expr = parse_expr("[]").expect("should parse");
        assert_eq!(expr, Expression::ListLiteral(vec![]));
    }

    #[test]
    fn expr_list_literal_integers() {
        let expr = parse_expr("[1, 2, 3]").expect("should parse");
        assert_eq!(
            expr,
            Expression::ListLiteral(vec![
                Expression::Literal(Literal::Integer(1)),
                Expression::Literal(Literal::Integer(2)),
                Expression::Literal(Literal::Integer(3)),
            ])
        );
    }

    #[test]
    fn expr_list_literal_mixed() {
        let expr = parse_expr("[1, 'hello', true, null]").expect("should parse");
        assert_eq!(
            expr,
            Expression::ListLiteral(vec![
                Expression::Literal(Literal::Integer(1)),
                Expression::Literal(Literal::String("hello".to_string())),
                Expression::Literal(Literal::Bool(true)),
                Expression::Literal(Literal::Null),
            ])
        );
    }

    #[test]
    fn expr_list_literal_nested() {
        let expr = parse_expr("[[1, 2], [3]]").expect("should parse");
        assert_eq!(
            expr,
            Expression::ListLiteral(vec![
                Expression::ListLiteral(vec![
                    Expression::Literal(Literal::Integer(1)),
                    Expression::Literal(Literal::Integer(2)),
                ]),
                Expression::ListLiteral(vec![Expression::Literal(Literal::Integer(3)),]),
            ])
        );
    }

    #[test]
    fn expr_list_literal_with_expressions() {
        let expr = parse_expr("[1 + 2, n.name]").expect("should parse");
        assert_eq!(
            expr,
            Expression::ListLiteral(vec![
                Expression::BinaryOp(
                    BinaryOp::Add,
                    Box::new(Expression::Literal(Literal::Integer(1))),
                    Box::new(Expression::Literal(Literal::Integer(2))),
                ),
                Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "name".to_string(),
                ),
            ])
        );
    }

    // All arithmetic operators
    #[test]
    fn expr_all_arithmetic() {
        let expr = parse_expr("1 + 2 - 3 * 4 / 5 % 6").expect("should parse");
        // 1 + 2 - ((3 * 4) / 5) % 6
        // Due to left-to-right at same precedence:
        // Mul/Div/Mod: 3*4=Mul(3,4), Mul(3,4)/5=Div(Mul(3,4),5), Div(...)/6... wait
        // Actually: 3 * 4 / 5 % 6 at precedence 9/10 (left-to-right)
        // = Mod(Div(Mul(3,4),5),6)
        // Then: 1 + 2 - Mod(...) at precedence 7/8 (left-to-right)
        // = Sub(Add(1,2), Mod(Div(Mul(3,4),5),6))
        assert_eq!(
            expr,
            Expression::BinaryOp(
                BinaryOp::Sub,
                Box::new(Expression::BinaryOp(
                    BinaryOp::Add,
                    Box::new(Expression::Literal(Literal::Integer(1))),
                    Box::new(Expression::Literal(Literal::Integer(2))),
                )),
                Box::new(Expression::BinaryOp(
                    BinaryOp::Mod,
                    Box::new(Expression::BinaryOp(
                        BinaryOp::Div,
                        Box::new(Expression::BinaryOp(
                            BinaryOp::Mul,
                            Box::new(Expression::Literal(Literal::Integer(3))),
                            Box::new(Expression::Literal(Literal::Integer(4))),
                        )),
                        Box::new(Expression::Literal(Literal::Integer(5))),
                    )),
                    Box::new(Expression::Literal(Literal::Integer(6))),
                )),
            )
        );
    }
}
