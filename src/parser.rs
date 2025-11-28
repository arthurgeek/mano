use crate::ast::Expr;
use crate::error::ManoError;
use crate::token::{Token, TokenType, Value};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Result<Expr, ManoError> {
        self.expression()
    }

    fn expression(&mut self) -> Result<Expr, ManoError> {
        self.comma()
    }

    fn comma(&mut self) -> Result<Expr, ManoError> {
        let mut expr = self.ternary()?;

        while self.match_types(&[TokenType::Comma]) {
            let operator = self.previous().clone();
            let right = self.ternary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn ternary(&mut self) -> Result<Expr, ManoError> {
        let expr = self.equality()?;

        if self.match_types(&[TokenType::Question]) {
            let then_branch = self.expression()?;
            self.consume(TokenType::Colon, "Cadê o ':' do ternário, mano?")?;
            let else_branch = self.ternary()?;
            return Ok(Expr::Ternary {
                condition: Box::new(expr),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            });
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, ManoError> {
        let mut expr = self.comparison()?;

        while self.match_types(&[TokenType::BangEqual, TokenType::EqualEqual]) {
            let operator = self.previous().clone();
            let right = self.comparison()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, ManoError> {
        let mut expr = self.term()?;

        while self.match_types(&[
            TokenType::Greater,
            TokenType::GreaterEqual,
            TokenType::Less,
            TokenType::LessEqual,
        ]) {
            let operator = self.previous().clone();
            let right = self.term()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, ManoError> {
        let mut expr = self.factor()?;

        while self.match_types(&[TokenType::Minus, TokenType::Plus]) {
            let operator = self.previous().clone();
            let right = self.factor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, ManoError> {
        let mut expr = self.unary()?;

        while self.match_types(&[TokenType::Slash, TokenType::Star]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, ManoError> {
        if self.match_types(&[TokenType::Bang, TokenType::Minus]) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            return Ok(Expr::Unary {
                operator,
                right: Box::new(right),
            });
        }
        self.primary()
    }

    fn match_types(&mut self, types: &[TokenType]) -> bool {
        for t in types {
            if self.check(t) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn primary(&mut self) -> Result<Expr, ManoError> {
        let token = self.peek();
        match token.token_type {
            TokenType::False => {
                self.advance();
                Ok(Expr::Literal {
                    value: Value::Bool(false),
                })
            }
            TokenType::True => {
                self.advance();
                Ok(Expr::Literal {
                    value: Value::Bool(true),
                })
            }
            TokenType::Nil => {
                self.advance();
                Ok(Expr::Literal { value: Value::Nil })
            }
            TokenType::Number | TokenType::String => {
                let value = token.literal.clone().unwrap();
                self.advance();
                Ok(Expr::Literal { value })
            }
            TokenType::LeftParen => {
                self.advance();
                let expr = self.expression()?;
                self.consume(TokenType::RightParen, "Cadê o fecha parênteses, mano?")?;
                Ok(Expr::Grouping {
                    expression: Box::new(expr),
                })
            }
            _ => Err(ManoError::Parse {
                line: token.line,
                message: "Cadê a expressão, mano?".to_string(),
            }),
        }
    }

    fn consume(&mut self, token_type: TokenType, message: &str) -> Result<&Token, ManoError> {
        if self.check(&token_type) {
            return Ok(self.advance());
        }
        Err(ManoError::Parse {
            line: self.peek().line,
            message: message.to_string(),
        })
    }

    #[allow(dead_code)]
    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.previous().token_type == TokenType::Semicolon {
                return;
            }

            match self.peek().token_type {
                TokenType::Class
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,
                _ => {}
            }

            self.advance();
        }
    }

    fn check(&self, token_type: &TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }
        &self.peek().token_type == token_type
    }

    // Helper methods

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().token_type == TokenType::Eof
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::{TokenType, Value};

    fn make_token(token_type: TokenType, lexeme: &str, literal: Option<Value>) -> Token {
        Token {
            token_type,
            lexeme: lexeme.to_string(),
            literal,
            line: 1,
        }
    }

    fn eof() -> Token {
        make_token(TokenType::Eof, "", None)
    }

    // === primary ===

    #[test]
    fn parses_number_literal() {
        let tokens = vec![
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Literal { value: Value::Number(n) } if n == 42.0));
    }

    #[test]
    fn parses_string_literal() {
        let tokens = vec![
            make_token(
                TokenType::String,
                "\"mano\"",
                Some(Value::String("mano".to_string())),
            ),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Literal { value: Value::String(ref s) } if s == "mano"));
    }

    #[test]
    fn parses_true_literal() {
        let tokens = vec![make_token(TokenType::True, "firmeza", None), eof()];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Value::Bool(true)
            }
        ));
    }

    #[test]
    fn parses_false_literal() {
        let tokens = vec![make_token(TokenType::False, "treta", None), eof()];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Value::Bool(false)
            }
        ));
    }

    #[test]
    fn parses_nil_literal() {
        let tokens = vec![make_token(TokenType::Nil, "nadaNão", None), eof()];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Literal { value: Value::Nil }));
    }

    #[test]
    fn parses_grouping() {
        let tokens = vec![
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            make_token(TokenType::RightParen, ")", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Grouping { .. }));
    }

    // === unary ===

    #[test]
    fn parses_unary_minus() {
        let tokens = vec![
            make_token(TokenType::Minus, "-", None),
            make_token(TokenType::Number, "5", Some(Value::Number(5.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Unary { .. }));
    }

    #[test]
    fn parses_unary_bang() {
        let tokens = vec![
            make_token(TokenType::Bang, "!", None),
            make_token(TokenType::True, "firmeza", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Unary { .. }));
    }

    // === factor ===

    #[test]
    fn parses_multiplication() {
        let tokens = vec![
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            make_token(TokenType::Star, "*", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    #[test]
    fn parses_division() {
        let tokens = vec![
            make_token(TokenType::Number, "6", Some(Value::Number(6.0))),
            make_token(TokenType::Slash, "/", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    // === term ===

    #[test]
    fn parses_addition() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    #[test]
    fn parses_subtraction() {
        let tokens = vec![
            make_token(TokenType::Number, "5", Some(Value::Number(5.0))),
            make_token(TokenType::Minus, "-", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    // === comparison ===

    #[test]
    fn parses_greater_than() {
        let tokens = vec![
            make_token(TokenType::Number, "5", Some(Value::Number(5.0))),
            make_token(TokenType::Greater, ">", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    #[test]
    fn parses_less_than() {
        let tokens = vec![
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            make_token(TokenType::Less, "<", None),
            make_token(TokenType::Number, "5", Some(Value::Number(5.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    // === equality ===

    #[test]
    fn parses_equal_equal() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::EqualEqual, "==", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    #[test]
    fn parses_bang_equal() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::BangEqual, "!=", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    // === errors ===

    #[test]
    fn error_on_unexpected_token() {
        let tokens = vec![make_token(TokenType::Plus, "+", None), eof()];
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        assert!(matches!(result, Err(ManoError::Parse { .. })));
    }

    #[test]
    fn error_on_unclosed_grouping() {
        let tokens = vec![
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        assert!(matches!(result, Err(ManoError::Parse { .. })));
    }

    // === comma ===

    #[test]
    fn parses_comma_expression() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Comma, ",", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Binary { .. }));
        assert_eq!(expr.to_string(), "(, 1 2)");
    }

    #[test]
    fn comma_is_left_associative() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Comma, ",", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            make_token(TokenType::Comma, ",", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        // Left associative: (1, 2), 3
        assert_eq!(expr.to_string(), "(, (, 1 2) 3)");
    }

    // === ternary ===

    #[test]
    fn parses_ternary_expression() {
        // firmeza ? 1 : 2
        let tokens = vec![
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::Question, "?", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Colon, ":", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert!(matches!(expr, Expr::Ternary { .. }));
        assert_eq!(expr.to_string(), "(?: firmeza 1 2)");
    }

    #[test]
    fn ternary_is_right_associative() {
        // a ? b : c ? d : e  =>  a ? b : (c ? d : e)
        let tokens = vec![
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::Question, "?", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Colon, ":", None),
            make_token(TokenType::False, "treta", None),
            make_token(TokenType::Question, "?", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            make_token(TokenType::Colon, ":", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let expr = parser.parse().unwrap();
        assert_eq!(expr.to_string(), "(?: firmeza 1 (?: treta 2 3))");
    }

    // === error productions ===

    #[test]
    fn error_on_binary_without_left_operand() {
        // * 3 should error but parse the 3
        let tokens = vec![
            make_token(TokenType::Star, "*", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        assert!(matches!(result, Err(ManoError::Parse { .. })));
    }

    #[test]
    fn error_on_plus_without_left_operand() {
        let tokens = vec![
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        assert!(matches!(result, Err(ManoError::Parse { .. })));
    }

    // === synchronize ===

    #[test]
    fn synchronize_skips_to_semicolon() {
        let tokens = vec![
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Star, "*", None),
            make_token(TokenType::Semicolon, ";", None),
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        parser.synchronize();
        assert_eq!(parser.peek().token_type, TokenType::Number);
    }

    #[test]
    fn synchronize_skips_to_statement_keyword() {
        let tokens = vec![
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Star, "*", None),
            make_token(TokenType::Var, "seLiga", None),
            make_token(TokenType::Identifier, "x", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        parser.synchronize();
        assert_eq!(parser.peek().token_type, TokenType::Var);
    }
}
