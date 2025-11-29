use crate::ast::{Expr, Stmt};
use crate::error::ManoError;
use crate::token::{Token, TokenType, Value};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    errors: Vec<ManoError>,
    loop_depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
            loop_depth: 0,
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, ManoError> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            if let Some(stmt) = self.declaration() {
                statements.push(stmt);
            }
        }
        Ok(statements)
    }

    pub fn take_errors(&mut self) -> Vec<ManoError> {
        std::mem::take(&mut self.errors)
    }

    fn declaration(&mut self) -> Option<Stmt> {
        let result = if self.match_types(&[TokenType::Var]) {
            self.var_declaration()
        } else {
            self.statement()
        };

        match result {
            Ok(stmt) => Some(stmt),
            Err(e) => {
                self.errors.push(e);
                self.synchronize();
                None
            }
        }
    }

    fn var_declaration(&mut self) -> Result<Stmt, ManoError> {
        let name = self
            .consume(TokenType::Identifier, "Cadê o nome da variável, parça?")?
            .clone();

        let initializer = if self.match_types(&[TokenType::Equal]) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            TokenType::Semicolon,
            "Cadê o ';' depois da declaração, véi?",
        )?;
        Ok(Stmt::Var { name, initializer })
    }

    fn statement(&mut self) -> Result<Stmt, ManoError> {
        if self.match_types(&[TokenType::Break]) {
            self.break_statement()
        } else if self.match_types(&[TokenType::For]) {
            self.for_statement()
        } else if self.match_types(&[TokenType::If]) {
            self.if_statement()
        } else if self.match_types(&[TokenType::While]) {
            self.while_statement()
        } else if self.match_types(&[TokenType::LeftBrace]) {
            self.block()
        } else if self.match_types(&[TokenType::Print]) {
            self.print_statement()
        } else {
            self.expression_statement()
        }
    }

    fn break_statement(&mut self) -> Result<Stmt, ManoError> {
        let keyword = self.previous().clone();
        if self.loop_depth == 0 {
            return Err(ManoError::Parse {
                message: "Não pode dar saiFora fora de um loop, mano!".to_string(),
                span: keyword.span,
            });
        }
        self.consume(TokenType::Semicolon, "Cadê o ';' depois do saiFora, véi?")?;
        Ok(Stmt::Break)
    }

    fn for_statement(&mut self) -> Result<Stmt, ManoError> {
        self.consume(TokenType::LeftParen, "Cadê o '(' depois do seVira, mano?")?;

        // Initializer
        let initializer = if self.match_types(&[TokenType::Semicolon]) {
            None
        } else if self.match_types(&[TokenType::Var]) {
            Some(self.var_declaration()?)
        } else {
            Some(self.expression_statement()?)
        };

        // Condition
        let condition = if self.check(&TokenType::Semicolon) {
            Expr::Literal {
                value: Value::Bool(true),
            }
        } else {
            self.expression()?
        };
        self.consume(TokenType::Semicolon, "Cadê o ';' depois da condição, véi?")?;

        // Increment
        let increment = if self.check(&TokenType::RightParen) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(TokenType::RightParen, "Cadê o ')' depois do seVira, mano?")?;

        // Body (inside loop context for break)
        self.loop_depth += 1;
        let body_result = self.statement();
        self.loop_depth -= 1;
        let mut body = body_result?;

        // Desugar: add increment to end of body
        if let Some(inc) = increment {
            body = Stmt::Block {
                statements: vec![body, Stmt::Expression { expression: inc }],
            };
        }

        // Desugar: wrap in while
        body = Stmt::While {
            condition,
            body: Box::new(body),
        };

        // Desugar: add initializer
        if let Some(init) = initializer {
            body = Stmt::Block {
                statements: vec![init, body],
            };
        }

        Ok(body)
    }

    fn while_statement(&mut self) -> Result<Stmt, ManoError> {
        self.consume(
            TokenType::LeftParen,
            "Cadê o '(' depois do segueOFluxo, mano?",
        )?;
        let condition = self.expression()?;
        self.consume(TokenType::RightParen, "Cadê o ')' depois da condição, véi?")?;

        self.loop_depth += 1;
        let body_result = self.statement();
        self.loop_depth -= 1;
        let body = Box::new(body_result?);

        Ok(Stmt::While { condition, body })
    }

    fn if_statement(&mut self) -> Result<Stmt, ManoError> {
        self.consume(TokenType::LeftParen, "Cadê o '(' depois do sePá, mano?")?;
        let condition = self.expression()?;
        self.consume(TokenType::RightParen, "Cadê o ')' depois da condição, véi?")?;

        let then_branch = Box::new(self.statement()?);
        let else_branch = if self.match_types(&[TokenType::Else]) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn block(&mut self) -> Result<Stmt, ManoError> {
        let mut statements = Vec::new();

        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            if let Some(stmt) = self.declaration() {
                statements.push(stmt);
            }
        }

        self.consume(
            TokenType::RightBrace,
            "Cadê o '}' pra fechar o bloco, mano?",
        )?;
        Ok(Stmt::Block { statements })
    }

    fn print_statement(&mut self) -> Result<Stmt, ManoError> {
        let expression = self.expression()?;
        self.consume(TokenType::Semicolon, "Cadê o ';' depois do salve, mano?")?;
        Ok(Stmt::Print { expression })
    }

    fn expression_statement(&mut self) -> Result<Stmt, ManoError> {
        let expression = self.expression()?;
        self.consume(TokenType::Semicolon, "Cadê o ';' no final, chapa?")?;
        Ok(Stmt::Expression { expression })
    }

    fn expression(&mut self) -> Result<Expr, ManoError> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr, ManoError> {
        let expr = self.comma()?;

        if self.match_types(&[TokenType::Equal]) {
            let equals = self.previous().clone();
            let value = self.assignment()?;

            if let Expr::Variable { name } = expr {
                return Ok(Expr::Assign {
                    name,
                    value: Box::new(value),
                });
            }

            return Err(ManoError::Parse {
                message: "Isso aí não dá pra atribuir, parça!".to_string(),
                span: equals.span.clone(),
            });
        }

        Ok(expr)
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
        let expr = self.or()?;

        if self.match_types(&[TokenType::Question]) {
            let then_branch = self.expression()?;
            self.consume(TokenType::Colon, "Cadê o ':' do ternário, chapa?")?;
            let else_branch = self.ternary()?;
            return Ok(Expr::Ternary {
                condition: Box::new(expr),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            });
        }

        Ok(expr)
    }

    fn or(&mut self) -> Result<Expr, ManoError> {
        let mut expr = self.and()?;

        while self.match_types(&[TokenType::Or]) {
            let operator = self.previous().clone();
            let right = self.and()?;
            expr = Expr::Logical {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, ManoError> {
        let mut expr = self.equality()?;

        while self.match_types(&[TokenType::And]) {
            let operator = self.previous().clone();
            let right = self.equality()?;
            expr = Expr::Logical {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
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
                self.consume(TokenType::RightParen, "Cadê o fecha parênteses, chegado?")?;
                Ok(Expr::Grouping {
                    expression: Box::new(expr),
                })
            }
            TokenType::Identifier => {
                let name = token.clone();
                self.advance();
                Ok(Expr::Variable { name })
            }
            _ => Err(ManoError::Parse {
                message: "Cadê a expressão, jão?".to_string(),
                span: token.span.clone(),
            }),
        }
    }

    fn consume(&mut self, token_type: TokenType, message: &str) -> Result<&Token, ManoError> {
        if self.check(&token_type) {
            return Ok(self.advance());
        }
        Err(ManoError::Parse {
            message: message.to_string(),
            span: self.peek().span.clone(),
        })
    }

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
    use crate::ast::Stmt;
    use crate::token::{TokenType, Value};

    fn make_token(token_type: TokenType, lexeme: &str, literal: Option<Value>) -> Token {
        Token {
            token_type,
            lexeme: lexeme.to_string(),
            literal,
            span: 0..lexeme.len(),
        }
    }

    fn eof() -> Token {
        make_token(TokenType::Eof, "", None)
    }

    fn semi() -> Token {
        make_token(TokenType::Semicolon, ";", None)
    }

    // === empty input ===

    #[test]
    fn parse_eof_only_returns_empty() {
        let tokens = vec![eof()];
        let mut parser = Parser::new(tokens);
        let result = parser.parse().unwrap();
        assert!(result.is_empty());
    }

    // === primary ===

    #[test]
    fn parses_number_literal() {
        let tokens = vec![
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(
                    matches!(expression, Expr::Literal { value: Value::Number(n) } if *n == 42.0)
                );
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_string_literal() {
        let tokens = vec![
            make_token(
                TokenType::String,
                "\"mano\"",
                Some(Value::String("mano".to_string())),
            ),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(
                    matches!(expression, Expr::Literal { value: Value::String(s) } if s == "mano")
                );
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_true_literal() {
        let tokens = vec![make_token(TokenType::True, "firmeza", None), semi(), eof()];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(
                    expression,
                    Expr::Literal {
                        value: Value::Bool(true)
                    }
                ));
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_false_literal() {
        let tokens = vec![make_token(TokenType::False, "treta", None), semi(), eof()];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(
                    expression,
                    Expr::Literal {
                        value: Value::Bool(false)
                    }
                ));
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_nil_literal() {
        let tokens = vec![make_token(TokenType::Nil, "nadaNão", None), semi(), eof()];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Literal { value: Value::Nil }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_grouping() {
        let tokens = vec![
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            make_token(TokenType::RightParen, ")", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Grouping { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === unary ===

    #[test]
    fn parses_unary_minus() {
        let tokens = vec![
            make_token(TokenType::Minus, "-", None),
            make_token(TokenType::Number, "5", Some(Value::Number(5.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Unary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_unary_bang() {
        let tokens = vec![
            make_token(TokenType::Bang, "!", None),
            make_token(TokenType::True, "firmeza", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Unary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === factor ===

    #[test]
    fn parses_multiplication() {
        let tokens = vec![
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            make_token(TokenType::Star, "*", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_division() {
        let tokens = vec![
            make_token(TokenType::Number, "6", Some(Value::Number(6.0))),
            make_token(TokenType::Slash, "/", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === term ===

    #[test]
    fn parses_addition() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_subtraction() {
        let tokens = vec![
            make_token(TokenType::Number, "5", Some(Value::Number(5.0))),
            make_token(TokenType::Minus, "-", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === comparison ===

    #[test]
    fn parses_greater_than() {
        let tokens = vec![
            make_token(TokenType::Number, "5", Some(Value::Number(5.0))),
            make_token(TokenType::Greater, ">", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_less_than() {
        let tokens = vec![
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            make_token(TokenType::Less, "<", None),
            make_token(TokenType::Number, "5", Some(Value::Number(5.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === equality ===

    #[test]
    fn parses_equal_equal() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::EqualEqual, "==", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_bang_equal() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::BangEqual, "!=", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === errors ===

    #[test]
    fn error_on_unexpected_token() {
        let tokens = vec![make_token(TokenType::Plus, "+", None), semi(), eof()];
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap();
        let errors = parser.take_errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Parse { .. }));
    }

    #[test]
    fn error_on_unclosed_grouping() {
        let tokens = vec![
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap();
        let errors = parser.take_errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Parse { .. }));
    }

    // === comma ===

    #[test]
    fn parses_comma_expression() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Comma, ",", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Binary { .. }));
                assert_eq!(expression.to_string(), "(, 1 2)");
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn comma_is_left_associative() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Comma, ",", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            make_token(TokenType::Comma, ",", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert_eq!(expression.to_string(), "(, (, 1 2) 3)");
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === ternary ===

    #[test]
    fn parses_ternary_expression() {
        let tokens = vec![
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::Question, "?", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Colon, ":", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Ternary { .. }));
                assert_eq!(expression.to_string(), "(?: firmeza 1 2)");
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn ternary_is_right_associative() {
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
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert_eq!(expression.to_string(), "(?: firmeza 1 (?: treta 2 3))");
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === error productions ===

    #[test]
    fn error_on_binary_without_left_operand() {
        let tokens = vec![
            make_token(TokenType::Star, "*", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap();
        let errors = parser.take_errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Parse { .. }));
    }

    #[test]
    fn error_on_plus_without_left_operand() {
        let tokens = vec![
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap();
        let errors = parser.take_errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Parse { .. }));
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

    // === variable declaration ===

    #[test]
    fn parses_var_declaration_with_initializer() {
        // seLiga x = 42;
        let tokens = vec![
            make_token(TokenType::Var, "seLiga", None),
            make_token(TokenType::Identifier, "x", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Var { name, initializer } => {
                assert_eq!(name.lexeme, "x");
                assert!(initializer.is_some());
            }
            _ => panic!("expected Var statement"),
        }
    }

    #[test]
    fn parses_var_declaration_without_initializer() {
        // seLiga x;
        let tokens = vec![
            make_token(TokenType::Var, "seLiga", None),
            make_token(TokenType::Identifier, "x", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Var { name, initializer } => {
                assert_eq!(name.lexeme, "x");
                assert!(initializer.is_none());
            }
            _ => panic!("expected Var statement"),
        }
    }

    // === assignment ===

    #[test]
    fn parses_assignment_expression() {
        // x = 42;
        let tokens = vec![
            make_token(TokenType::Identifier, "x", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Assign { name, .. } if name.lexeme == "x"));
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === variable expression ===

    #[test]
    fn parses_variable_expression() {
        // x;
        let tokens = vec![make_token(TokenType::Identifier, "x", None), semi(), eof()];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Variable { name } if name.lexeme == "x"));
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === error recovery ===

    #[test]
    fn recovers_from_parse_error_and_continues() {
        // First statement has error, second is valid
        // seLiga = 42; salve 1;
        let tokens = vec![
            make_token(TokenType::Var, "seLiga", None),
            make_token(TokenType::Equal, "=", None), // error: missing identifier
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            semi(),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        // Should still parse the second statement
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1); // Only the valid statement
        assert!(matches!(stmts[0], Stmt::Print { .. }));
    }

    // === statements ===

    #[test]
    fn parses_print_statement() {
        let tokens = vec![
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "42", Some(Value::Number(42.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0], Stmt::Print { .. }));
    }

    #[test]
    fn parses_multiple_statements() {
        // salve 1; salve 2;
        let tokens = vec![
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 2);
        assert!(matches!(stmts[0], Stmt::Print { .. }));
        assert!(matches!(stmts[1], Stmt::Print { .. }));
    }

    // === block statements ===

    #[test]
    fn parses_empty_block() {
        // { }
        let tokens = vec![
            make_token(TokenType::LeftBrace, "{", None),
            make_token(TokenType::RightBrace, "}", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Block { statements } if statements.is_empty()));
    }

    #[test]
    fn parses_block_with_statements() {
        // { salve 1; salve 2; }
        let tokens = vec![
            make_token(TokenType::LeftBrace, "{", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            make_token(TokenType::RightBrace, "}", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Block { statements } => {
                assert_eq!(statements.len(), 2);
                assert!(
                    matches!(&statements[0], Stmt::Print { expression: Expr::Literal { value: Value::Number(n) } } if *n == 1.0)
                );
                assert!(
                    matches!(&statements[1], Stmt::Print { expression: Expr::Literal { value: Value::Number(n) } } if *n == 2.0)
                );
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn parses_nested_blocks() {
        // { { salve 1; } }
        let tokens = vec![
            make_token(TokenType::LeftBrace, "{", None),
            make_token(TokenType::LeftBrace, "{", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            make_token(TokenType::RightBrace, "}", None),
            make_token(TokenType::RightBrace, "}", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Block { statements } => {
                assert_eq!(statements.len(), 1);
                assert!(matches!(&statements[0], Stmt::Block { .. }));
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn error_on_invalid_assignment_target() {
        // 1 = 2; (can't assign to a literal)
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap();
        let errors = parser.take_errors();
        assert_eq!(errors.len(), 1);
        if let ManoError::Parse { message, .. } = &errors[0] {
            assert!(message.contains("atribuir"));
        } else {
            panic!("Expected Parse error");
        }
    }

    #[test]
    fn take_errors_returns_and_clears_errors() {
        let tokens = vec![
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap();

        let errors = parser.take_errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Parse { .. }));

        // Errors should be cleared
        assert!(parser.take_errors().is_empty());
    }

    // === if statements ===

    #[test]
    fn parses_if_statement() {
        // sePá (firmeza) salve 1;
        let tokens = vec![
            make_token(TokenType::If, "sePá", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                assert!(matches!(
                    condition,
                    Expr::Literal {
                        value: Value::Bool(true)
                    }
                ));
                assert!(matches!(then_branch.as_ref(), Stmt::Print { .. }));
                assert!(else_branch.is_none());
            }
            _ => panic!("expected If statement"),
        }
    }

    #[test]
    fn parses_if_else_statement() {
        // sePá (treta) salve 1; vacilou salve 2;
        let tokens = vec![
            make_token(TokenType::If, "sePá", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::False, "treta", None),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            make_token(TokenType::Else, "vacilou", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "2", Some(Value::Number(2.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                assert!(matches!(
                    condition,
                    Expr::Literal {
                        value: Value::Bool(false)
                    }
                ));
                assert!(matches!(then_branch.as_ref(), Stmt::Print { .. }));
                assert!(else_branch.is_some());
                assert!(matches!(
                    else_branch.as_ref().unwrap().as_ref(),
                    Stmt::Print { .. }
                ));
            }
            _ => panic!("expected If statement"),
        }
    }

    #[test]
    fn parses_if_with_block() {
        // sePá (firmeza) { salve 1; }
        let tokens = vec![
            make_token(TokenType::If, "sePá", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::LeftBrace, "{", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            make_token(TokenType::RightBrace, "}", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::If { then_branch, .. } => {
                assert!(matches!(then_branch.as_ref(), Stmt::Block { .. }));
            }
            _ => panic!("expected If statement"),
        }
    }

    // === logical operators ===

    #[test]
    fn parses_or_expression() {
        // firmeza ow treta;
        let tokens = vec![
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::Or, "ow", None),
            make_token(TokenType::False, "treta", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Logical { .. }));
                assert_eq!(expression.to_string(), "(ow firmeza treta)");
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parses_and_expression() {
        // firmeza tamoJunto treta;
        let tokens = vec![
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::And, "tamoJunto", None),
            make_token(TokenType::False, "treta", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert!(matches!(expression, Expr::Logical { .. }));
                assert_eq!(expression.to_string(), "(tamoJunto firmeza treta)");
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn and_has_higher_precedence_than_or() {
        // a ow b tamoJunto c -> a ow (b tamoJunto c)
        let tokens = vec![
            make_token(TokenType::Identifier, "a", None),
            make_token(TokenType::Or, "ow", None),
            make_token(TokenType::Identifier, "b", None),
            make_token(TokenType::And, "tamoJunto", None),
            make_token(TokenType::Identifier, "c", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert_eq!(expression.to_string(), "(ow a (tamoJunto b c))");
            }
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn or_is_left_associative() {
        // a ow b ow c -> (a ow b) ow c
        let tokens = vec![
            make_token(TokenType::Identifier, "a", None),
            make_token(TokenType::Or, "ow", None),
            make_token(TokenType::Identifier, "b", None),
            make_token(TokenType::Or, "ow", None),
            make_token(TokenType::Identifier, "c", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        match &stmts[0] {
            Stmt::Expression { expression } => {
                assert_eq!(expression.to_string(), "(ow (ow a b) c)");
            }
            _ => panic!("expected expression statement"),
        }
    }

    // === while statements ===

    #[test]
    fn parses_while_statement() {
        // segueOFluxo (firmeza) salve 1;
        let tokens = vec![
            make_token(TokenType::While, "segueOFluxo", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::While { condition, body } => {
                assert!(matches!(
                    condition,
                    Expr::Literal {
                        value: Value::Bool(true)
                    }
                ));
                assert!(matches!(body.as_ref(), Stmt::Print { .. }));
            }
            _ => panic!("expected While statement"),
        }
    }

    #[test]
    fn parses_while_with_block() {
        // segueOFluxo (firmeza) { salve 1; }
        let tokens = vec![
            make_token(TokenType::While, "segueOFluxo", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::LeftBrace, "{", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            semi(),
            make_token(TokenType::RightBrace, "}", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::While { body, .. } => {
                assert!(matches!(body.as_ref(), Stmt::Block { .. }));
            }
            _ => panic!("expected While statement"),
        }
    }

    // === for statements ===

    #[test]
    fn parses_for_with_all_clauses() {
        // seVira (seLiga i = 0; i < 3; i = i + 1) salve i;
        // Desugars to: { seLiga i = 0; segueOFluxo (i < 3) { salve i; i = i + 1; } }
        let tokens = vec![
            make_token(TokenType::For, "seVira", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::Var, "seLiga", None),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Number, "0", Some(Value::Number(0.0))),
            semi(),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Less, "<", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Identifier, "i", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        // Should be a block containing var decl and while
        match &stmts[0] {
            Stmt::Block { statements } => {
                assert_eq!(statements.len(), 2);
                assert!(matches!(&statements[0], Stmt::Var { .. }));
                assert!(matches!(&statements[1], Stmt::While { .. }));
            }
            _ => panic!("expected Block statement (desugared for)"),
        }
    }

    #[test]
    fn parses_for_without_initializer() {
        // seVira (; i < 3; i = i + 1) salve i;
        let tokens = vec![
            make_token(TokenType::For, "seVira", None),
            make_token(TokenType::LeftParen, "(", None),
            semi(),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Less, "<", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Identifier, "i", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        // No initializer means just a while loop (no wrapping block)
        assert!(matches!(&stmts[0], Stmt::While { .. }));
    }

    #[test]
    fn parses_for_without_condition() {
        // seVira (seLiga i = 0;; i = i + 1) salve i;
        // Infinite loop - condition defaults to true
        let tokens = vec![
            make_token(TokenType::For, "seVira", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::Var, "seLiga", None),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Number, "0", Some(Value::Number(0.0))),
            semi(),
            semi(), // empty condition
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Identifier, "i", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Block { statements } => {
                assert_eq!(statements.len(), 2);
                match &statements[1] {
                    Stmt::While { condition, .. } => {
                        // Condition should be true literal
                        assert!(matches!(
                            condition,
                            Expr::Literal {
                                value: Value::Bool(true)
                            }
                        ));
                    }
                    _ => panic!("expected While"),
                }
            }
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn parses_for_with_expression_initializer() {
        // seVira (i = 0; i < 3; i = i + 1) salve i;
        // Expression initializer (not var declaration)
        let tokens = vec![
            make_token(TokenType::For, "seVira", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Number, "0", Some(Value::Number(0.0))),
            semi(),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Less, "<", None),
            make_token(TokenType::Number, "3", Some(Value::Number(3.0))),
            semi(),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Equal, "=", None),
            make_token(TokenType::Identifier, "i", None),
            make_token(TokenType::Plus, "+", None),
            make_token(TokenType::Number, "1", Some(Value::Number(1.0))),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Print, "salve", None),
            make_token(TokenType::Identifier, "i", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        // Should desugar to block with expression statement + while
        match &stmts[0] {
            Stmt::Block { statements } => {
                assert_eq!(statements.len(), 2);
                // First should be expression statement (i = 0)
                assert!(matches!(&statements[0], Stmt::Expression { .. }));
                // Second should be while
                assert!(matches!(&statements[1], Stmt::While { .. }));
            }
            _ => panic!("expected Block"),
        }
    }

    // === break statements ===

    #[test]
    fn parses_break_in_while() {
        // segueOFluxo (firmeza) saiFora;
        let tokens = vec![
            make_token(TokenType::While, "segueOFluxo", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Break, "saiFora", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::While { body, .. } => {
                assert!(matches!(body.as_ref(), Stmt::Break));
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn parses_break_in_for() {
        // seVira (;;) saiFora;
        let tokens = vec![
            make_token(TokenType::For, "seVira", None),
            make_token(TokenType::LeftParen, "(", None),
            semi(),
            semi(),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Break, "saiFora", None),
            semi(),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        assert_eq!(stmts.len(), 1);
        // for desugars to while
        match &stmts[0] {
            Stmt::While { body, .. } => {
                assert!(matches!(body.as_ref(), Stmt::Break));
            }
            _ => panic!("expected While (desugared for)"),
        }
    }

    #[test]
    fn break_outside_loop_is_error() {
        // saiFora; (not in a loop)
        let tokens = vec![make_token(TokenType::Break, "saiFora", None), semi(), eof()];
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap();
        let errors = parser.take_errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Parse { .. }));
    }

    #[test]
    fn break_in_nested_if_inside_loop_is_ok() {
        // segueOFluxo (firmeza) { sePá (firmeza) saiFora; }
        let tokens = vec![
            make_token(TokenType::While, "segueOFluxo", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::LeftBrace, "{", None),
            make_token(TokenType::If, "sePá", None),
            make_token(TokenType::LeftParen, "(", None),
            make_token(TokenType::True, "firmeza", None),
            make_token(TokenType::RightParen, ")", None),
            make_token(TokenType::Break, "saiFora", None),
            semi(),
            make_token(TokenType::RightBrace, "}", None),
            eof(),
        ];
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse().unwrap();
        let errors = parser.take_errors();
        assert!(errors.is_empty());
        assert_eq!(stmts.len(), 1);
    }
}
