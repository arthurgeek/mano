use crate::error::ManoError;
use crate::token::{Token, TokenType, Value};

pub struct Scanner<'a> {
    source: &'a str,
    start: usize,
    current: usize,
    line: usize,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            line: 1,
        }
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Result<Token, ManoError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current > self.source.len() {
                return None;
            }

            if self.is_at_end() {
                self.current += 1;
                return Some(Ok(Token {
                    token_type: TokenType::Eof,
                    lexeme: String::new(),
                    literal: None,
                    line: self.line,
                }));
            }

            self.start = self.current;
            let c = self.advance();

            match c {
                // Whitespace
                ' ' | '\r' | '\t' => continue,
                '\n' => {
                    self.line += 1;
                    continue;
                }
                // Single-character tokens
                '(' => return Some(Ok(self.add_token(TokenType::LeftParen))),
                ')' => return Some(Ok(self.add_token(TokenType::RightParen))),
                '{' => return Some(Ok(self.add_token(TokenType::LeftBrace))),
                '}' => return Some(Ok(self.add_token(TokenType::RightBrace))),
                ',' => return Some(Ok(self.add_token(TokenType::Comma))),
                '.' => return Some(Ok(self.add_token(TokenType::Dot))),
                '-' => return Some(Ok(self.add_token(TokenType::Minus))),
                '+' => return Some(Ok(self.add_token(TokenType::Plus))),
                ';' => return Some(Ok(self.add_token(TokenType::Semicolon))),
                '?' => return Some(Ok(self.add_token(TokenType::Question))),
                ':' => return Some(Ok(self.add_token(TokenType::Colon))),
                // Slash or comment
                '/' => {
                    if self.match_char('/') {
                        // Line comment - consume until end of line
                        while self.peek() != Some('\n') && !self.is_at_end() {
                            self.advance();
                        }
                        continue;
                    } else if self.match_char('*') {
                        // Block comment - consume until */
                        if let Err(e) = self.block_comment() {
                            return Some(Err(e));
                        }
                        continue;
                    } else {
                        return Some(Ok(self.add_token(TokenType::Slash)));
                    }
                }
                '*' => return Some(Ok(self.add_token(TokenType::Star))),
                '!' => {
                    let token_type = if self.match_char('=') {
                        TokenType::BangEqual
                    } else {
                        TokenType::Bang
                    };
                    return Some(Ok(self.add_token(token_type)));
                }
                '=' => {
                    let token_type = if self.match_char('=') {
                        TokenType::EqualEqual
                    } else {
                        TokenType::Equal
                    };
                    return Some(Ok(self.add_token(token_type)));
                }
                '<' => {
                    let token_type = if self.match_char('=') {
                        TokenType::LessEqual
                    } else {
                        TokenType::Less
                    };
                    return Some(Ok(self.add_token(token_type)));
                }
                '>' => {
                    let token_type = if self.match_char('=') {
                        TokenType::GreaterEqual
                    } else {
                        TokenType::Greater
                    };
                    return Some(Ok(self.add_token(token_type)));
                }
                '"' => return Some(self.string()),
                c if c.is_ascii_digit() => return Some(Ok(self.number())),
                c if c.is_alphabetic() || c == '_' => return Some(Ok(self.identifier())),
                _ => {
                    return Some(Err(ManoError::UnexpectedCharacter {
                        line: self.line,
                        lexeme: c,
                    }));
                }
            }
        }
    }
}

impl<'a> Scanner<'a> {
    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current..].chars().next().unwrap();
        self.current += c.len_utf8();
        c
    }

    fn peek(&self) -> Option<char> {
        self.source[self.current..].chars().next()
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn add_token(&self, token_type: TokenType) -> Token {
        Token {
            token_type,
            lexeme: self.source[self.start..self.current].to_string(),
            literal: None,
            line: self.line,
        }
    }

    fn add_token_with_literal(&self, token_type: TokenType, literal: Value) -> Token {
        Token {
            token_type,
            lexeme: self.source[self.start..self.current].to_string(),
            literal: Some(literal),
            line: self.line,
        }
    }

    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.current..].chars();
        chars.next(); // skip current
        chars.next() // return next
    }

    fn identifier(&mut self) -> Token {
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            self.advance();
        }

        let text = &self.source[self.start..self.current];
        let token_type = Self::keyword(text).unwrap_or(TokenType::Identifier);
        self.add_token(token_type)
    }

    fn keyword(text: &str) -> Option<TokenType> {
        match text {
            "tamoJunto" => Some(TokenType::And),
            "bagulho" => Some(TokenType::Class),
            "vacilou" => Some(TokenType::Else),
            "treta" => Some(TokenType::False),
            "olhaEssaFita" => Some(TokenType::Fun),
            "seVira" => Some(TokenType::For),
            "sePá" => Some(TokenType::If),
            "nadaNão" => Some(TokenType::Nil),
            "ow" => Some(TokenType::Or),
            "salve" | "oiSumida" => Some(TokenType::Print),
            "toma" => Some(TokenType::Return),
            "mestre" => Some(TokenType::Super),
            "oCara" => Some(TokenType::This),
            "firmeza" => Some(TokenType::True),
            "seLiga" => Some(TokenType::Var),
            "segueOFluxo" => Some(TokenType::While),
            "saiFora" => Some(TokenType::Break),
            _ => None,
        }
    }

    fn number(&mut self) -> Token {
        // Consume digits
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }

        // Look for decimal part - only if dot is followed by digit
        if self.peek() == Some('.') && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
            self.advance(); // consume the '.'
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }

        let value: f64 = self.source[self.start..self.current].parse().unwrap();
        self.add_token_with_literal(TokenType::Number, Value::Number(value))
    }

    fn string(&mut self) -> Result<Token, ManoError> {
        // Consume characters until closing quote
        while self.peek() != Some('"') && !self.is_at_end() {
            if self.peek() == Some('\n') {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            return Err(ManoError::UnterminatedString { line: self.line });
        }

        // Consume the closing "
        self.advance();

        // Extract the string value (without quotes)
        let value = self.source[self.start + 1..self.current - 1].to_string();
        Ok(self.add_token_with_literal(TokenType::String, Value::String(value)))
    }

    fn block_comment(&mut self) -> Result<(), ManoError> {
        let start_line = self.line;
        let mut depth = 1;

        while depth > 0 && !self.is_at_end() {
            let c = self.advance();

            if c == '\n' {
                self.line += 1;
            } else if c == '/' && self.peek() == Some('*') {
                self.advance(); // consume '*'
                depth += 1;
            } else if c == '*' && self.peek() == Some('/') {
                self.advance(); // consume '/'
                depth -= 1;
            }
        }

        if depth > 0 {
            return Err(ManoError::UnterminatedBlockComment { line: start_line });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source_returns_eof() {
        let mut scanner = Scanner::new("");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::Eof);
        assert!(scanner.next().is_none());
    }

    #[test]
    fn scans_left_paren() {
        let mut scanner = Scanner::new("(");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::LeftParen);
        assert_eq!(token.lexeme, "(");
    }

    #[test]
    fn tracks_line_numbers() {
        let mut scanner = Scanner::new("(\n)");

        let token1 = scanner.next().unwrap().unwrap();
        assert_eq!(token1.line, 1);

        let token2 = scanner.next().unwrap().unwrap();
        assert_eq!(token2.token_type, TokenType::RightParen);
        assert_eq!(token2.line, 2);
    }

    #[test]
    fn is_at_end_returns_false_when_not_at_end() {
        let scanner = Scanner::new("(");
        assert!(!scanner.is_at_end());
    }

    #[test]
    fn is_at_end_returns_true_for_empty_source() {
        let scanner = Scanner::new("");
        assert!(scanner.is_at_end());
    }

    #[test]
    fn returns_error_for_unexpected_characters() {
        use crate::error::ManoError;

        let mut scanner = Scanner::new("(@)");

        let first = scanner.next().unwrap();
        assert!(first.is_ok());
        assert_eq!(first.unwrap().token_type, TokenType::LeftParen);

        let second = scanner.next().unwrap();
        assert!(second.is_err());
        assert!(matches!(
            second.unwrap_err(),
            ManoError::UnexpectedCharacter {
                line: 1,
                lexeme: '@'
            }
        ));

        let third = scanner.next().unwrap();
        assert!(third.is_ok());
        assert_eq!(third.unwrap().token_type, TokenType::RightParen);
    }

    #[test]
    fn scans_all_single_char_tokens() {
        let mut scanner = Scanner::new("(){},.-+;?:*/");

        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::LeftParen
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::RightParen
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::LeftBrace
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::RightBrace
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::Comma
        );
        assert_eq!(scanner.next().unwrap().unwrap().token_type, TokenType::Dot);
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::Minus
        );
        assert_eq!(scanner.next().unwrap().unwrap().token_type, TokenType::Plus);
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::Semicolon
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::Question
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::Colon
        );
        assert_eq!(scanner.next().unwrap().unwrap().token_type, TokenType::Star);
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::Slash
        );
        assert_eq!(scanner.next().unwrap().unwrap().token_type, TokenType::Eof);
    }

    #[test]
    fn scans_equal() {
        let mut scanner = Scanner::new("=");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::Equal);
        assert_eq!(token.lexeme, "=");
    }

    #[test]
    fn scans_less() {
        let mut scanner = Scanner::new("<");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::Less);
        assert_eq!(token.lexeme, "<");
    }

    #[test]
    fn scans_greater() {
        let mut scanner = Scanner::new(">");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::Greater);
        assert_eq!(token.lexeme, ">");
    }

    #[test]
    fn scans_bang() {
        let mut scanner = Scanner::new("!");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::Bang);
        assert_eq!(token.lexeme, "!");
    }

    #[test]
    fn scans_bang_equal() {
        let mut scanner = Scanner::new("!=");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::BangEqual);
        assert_eq!(token.lexeme, "!=");
    }

    #[test]
    fn scans_equal_equal() {
        let mut scanner = Scanner::new("==");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::EqualEqual);
        assert_eq!(token.lexeme, "==");
    }

    #[test]
    fn scans_less_equal() {
        let mut scanner = Scanner::new("<=");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::LessEqual);
        assert_eq!(token.lexeme, "<=");
    }

    #[test]
    fn scans_greater_equal() {
        let mut scanner = Scanner::new(">=");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::GreaterEqual);
        assert_eq!(token.lexeme, ">=");
    }

    #[test]
    fn skips_spaces() {
        let mut scanner = Scanner::new("( )");

        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::LeftParen
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::RightParen
        );
    }

    #[test]
    fn skips_tabs() {
        let mut scanner = Scanner::new("(\t)");

        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::LeftParen
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::RightParen
        );
    }

    #[test]
    fn skips_carriage_return() {
        let mut scanner = Scanner::new("(\r)");

        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::LeftParen
        );
        assert_eq!(
            scanner.next().unwrap().unwrap().token_type,
            TokenType::RightParen
        );
    }

    #[test]
    fn skips_line_comments() {
        let mut scanner = Scanner::new("( // this is a comment\n)");

        let first = scanner.next().unwrap().unwrap();
        assert_eq!(first.token_type, TokenType::LeftParen);

        let second = scanner.next().unwrap().unwrap();
        assert_eq!(second.token_type, TokenType::RightParen);
        assert_eq!(second.line, 2);
    }

    #[test]
    fn comment_at_end_of_file() {
        let mut scanner = Scanner::new("( // comment");

        let first = scanner.next().unwrap().unwrap();
        assert_eq!(first.token_type, TokenType::LeftParen);

        let second = scanner.next().unwrap().unwrap();
        assert_eq!(second.token_type, TokenType::Eof);
    }

    #[test]
    fn scans_string_literal() {
        use crate::token::Value;

        let mut scanner = Scanner::new("\"mano\"");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::String);
        assert_eq!(token.lexeme, "\"mano\"");
        assert_eq!(token.literal, Some(Value::String("mano".to_string())));
    }

    #[test]
    fn scans_string_literal_with_unicode() {
        use crate::token::Value;

        let mut scanner = Scanner::new("\"e aí mano, beleza?\"");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::String);
        assert_eq!(token.lexeme, "\"e aí mano, beleza?\"");
        assert_eq!(
            token.literal,
            Some(Value::String("e aí mano, beleza?".to_string()))
        );
    }

    #[test]
    fn unterminated_string_returns_error() {
        let mut scanner = Scanner::new("\"esqueceu de fechar");
        let result = scanner.next().unwrap();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ManoError::UnterminatedString { line: 1 }
        ));
    }

    #[test]
    fn scans_multiline_string() {
        use crate::token::Value;

        let mut scanner = Scanner::new("\"primeira linha\nsegunda linha\"");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::String);
        assert_eq!(
            token.literal,
            Some(Value::String("primeira linha\nsegunda linha".to_string()))
        );
        assert_eq!(token.line, 2); // Line should be updated

        // Next token (Eof) should be on line 2
        let eof = scanner.next().unwrap().unwrap();
        assert_eq!(eof.line, 2);
    }

    #[test]
    fn scans_integer_literal() {
        let mut scanner = Scanner::new("1234");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Number);
        assert_eq!(token.lexeme, "1234");
        assert_eq!(token.literal, Some(Value::Number(1234.0)));
    }

    #[test]
    fn scans_decimal_literal() {
        let mut scanner = Scanner::new("12.34");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Number);
        assert_eq!(token.lexeme, "12.34");
        assert_eq!(token.literal, Some(Value::Number(12.34)));
    }

    #[test]
    fn trailing_dot_is_not_decimal() {
        // "1234." should be number 1234 followed by dot
        let mut scanner = Scanner::new("1234.");

        let num = scanner.next().unwrap().unwrap();
        assert_eq!(num.token_type, TokenType::Number);
        assert_eq!(num.literal, Some(Value::Number(1234.0)));

        let dot = scanner.next().unwrap().unwrap();
        assert_eq!(dot.token_type, TokenType::Dot);
    }

    #[test]
    fn leading_dot_is_not_decimal() {
        // ".1234" should be dot followed by number 1234
        let mut scanner = Scanner::new(".1234");

        let dot = scanner.next().unwrap().unwrap();
        assert_eq!(dot.token_type, TokenType::Dot);

        let num = scanner.next().unwrap().unwrap();
        assert_eq!(num.token_type, TokenType::Number);
        assert_eq!(num.literal, Some(Value::Number(1234.0)));
    }

    #[test]
    fn scans_identifier() {
        let mut scanner = Scanner::new("meuNome");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Identifier);
        assert_eq!(token.lexeme, "meuNome");
    }

    #[test]
    fn scans_identifier_with_underscore() {
        let mut scanner = Scanner::new("_meu_nome_123");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Identifier);
        assert_eq!(token.lexeme, "_meu_nome_123");
    }

    #[test]
    fn scans_identifier_with_unicode() {
        let mut scanner = Scanner::new("variável");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Identifier);
        assert_eq!(token.lexeme, "variável");
    }

    #[test]
    #[allow(non_snake_case)]
    fn scans_keyword_seLiga() {
        let mut scanner = Scanner::new("seLiga");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Var);
        assert_eq!(token.lexeme, "seLiga");
    }

    #[test]
    fn scans_keyword_firmeza() {
        let mut scanner = Scanner::new("firmeza");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::True);
    }

    #[test]
    fn scans_keyword_treta() {
        let mut scanner = Scanner::new("treta");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::False);
    }

    #[test]
    #[allow(non_snake_case)]
    fn scans_keyword_nadaNão() {
        let mut scanner = Scanner::new("nadaNão");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Nil);
    }

    #[test]
    fn skips_block_comments() {
        let scanner = Scanner::new("( /* comentário */ )");
        let tokens: Vec<_> = scanner.collect();

        assert_eq!(tokens.len(), 3); // ( ) Eof
        assert_eq!(tokens[0].as_ref().unwrap().token_type, TokenType::LeftParen);
        assert_eq!(
            tokens[1].as_ref().unwrap().token_type,
            TokenType::RightParen
        );
        assert_eq!(tokens[2].as_ref().unwrap().token_type, TokenType::Eof);
    }

    #[test]
    fn skips_multiline_block_comments() {
        let scanner = Scanner::new("(\n/* linha 1\nlinha 2\nlinha 3 */\n)");
        let tokens: Vec<_> = scanner.collect();

        assert_eq!(tokens.len(), 3);
        // Line number should be updated after multiline comment
        assert_eq!(tokens[1].as_ref().unwrap().line, 5);
    }

    #[test]
    fn skips_nested_block_comments() {
        let scanner = Scanner::new("( /* outer /* inner */ still outer */ )");
        let tokens: Vec<_> = scanner.collect();

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].as_ref().unwrap().token_type, TokenType::LeftParen);
        assert_eq!(
            tokens[1].as_ref().unwrap().token_type,
            TokenType::RightParen
        );
    }

    #[test]
    fn unterminated_block_comment_returns_error() {
        let scanner = Scanner::new("( /* comentário sem fim");
        let tokens: Vec<_> = scanner.collect();

        // Should have LeftParen, Error, Eof
        let error = tokens.iter().find(|t| t.is_err()).unwrap();
        assert!(matches!(
            error,
            Err(ManoError::UnterminatedBlockComment { .. })
        ));
    }
}
