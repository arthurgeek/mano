use crate::error::ManoError;
use crate::token::{Literal, Token, TokenType};
use unicode_properties::UnicodeEmoji;

/// Check if a character can start an identifier
pub fn is_identifier_start(c: char) -> bool {
    !c.is_ascii_digit() && (c.is_alphabetic() || c == '_' || c.is_emoji_char())
}

/// Check if a character can continue an identifier
pub fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c.is_emoji_char()
}

/// All mano keywords with their token types
pub const KEYWORDS: &[(&str, TokenType)] = &[
    ("bagulho", TokenType::Class),
    ("firmeza", TokenType::True),
    ("mestre", TokenType::Super),
    ("nadaNão", TokenType::Nil),
    ("oCara", TokenType::This),
    ("oiSumida", TokenType::Print),
    ("olhaEssaFita", TokenType::Fun),
    ("ow", TokenType::Or),
    ("saiFora", TokenType::Break),
    ("salve", TokenType::Print),
    ("seLiga", TokenType::Var),
    ("sePá", TokenType::If),
    ("seVira", TokenType::For),
    ("segueOFluxo", TokenType::While),
    ("tamoJunto", TokenType::And),
    ("toma", TokenType::Return),
    ("treta", TokenType::False),
    ("vacilou", TokenType::Else),
];

pub struct Scanner<'a> {
    source: &'a str,
    start: usize,
    current: usize,
    include_comments: bool,
    /// Stack of brace depths for nested string interpolations.
    /// Each entry represents an interpolated string we're inside.
    /// The value is the brace nesting depth within that interpolation.
    interpolation_stack: Vec<usize>,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            include_comments: false,
            interpolation_stack: Vec::new(),
        }
    }

    /// Create a scanner that includes comment tokens (for highlighting)
    pub fn with_comments(source: &'a str) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            include_comments: true,
            interpolation_stack: Vec::new(),
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
                let span = self.current..self.current;
                self.current += 1;
                return Some(Ok(Token {
                    token_type: TokenType::Eof,
                    lexeme: String::new(),
                    literal: None,
                    span,
                }));
            }

            self.start = self.current;
            let c = self.advance();

            match c {
                // Whitespace
                ' ' | '\r' | '\t' | '\n' => continue,
                // Single-character tokens
                '(' => return Some(Ok(self.add_token(TokenType::LeftParen))),
                ')' => return Some(Ok(self.add_token(TokenType::RightParen))),
                '{' => {
                    // Track brace depth for interpolation
                    if let Some(depth) = self.interpolation_stack.last_mut() {
                        *depth += 1;
                    }
                    return Some(Ok(self.add_token(TokenType::LeftBrace)));
                }
                '}' => {
                    // Check if this closes an interpolation
                    if let Some(depth) = self.interpolation_stack.last_mut() {
                        if *depth == 0 {
                            // This closes the interpolation - continue scanning string
                            self.interpolation_stack.pop();
                            return Some(self.interpolated_string_continue());
                        } else {
                            *depth -= 1;
                        }
                    }
                    return Some(Ok(self.add_token(TokenType::RightBrace)));
                }
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
                        if self.include_comments {
                            return Some(Ok(self.add_token(TokenType::Comment)));
                        }
                        continue;
                    } else if self.match_char('*') {
                        // Block comment - consume until */
                        if let Err(e) = self.block_comment() {
                            return Some(Err(e));
                        }
                        if self.include_comments {
                            return Some(Ok(self.add_token(TokenType::Comment)));
                        }
                        continue;
                    } else {
                        return Some(Ok(self.add_token(TokenType::Slash)));
                    }
                }
                '*' => return Some(Ok(self.add_token(TokenType::Star))),
                '%' => return Some(Ok(self.add_token(TokenType::Percent))),
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
                c if is_identifier_start(c) => {
                    return Some(Ok(self.identifier()));
                }
                _ => {
                    return Some(Err(ManoError::Scan {
                        message: format!("E esse '{}' aí, truta?", c),
                        span: self.start..self.current,
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
            span: self.start..self.current,
        }
    }

    fn add_token_with_literal(&self, token_type: TokenType, literal: Literal) -> Token {
        Token {
            token_type,
            lexeme: self.source[self.start..self.current].to_string(),
            literal: Some(literal),
            span: self.start..self.current,
        }
    }

    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.current..].chars();
        chars.next(); // skip current
        chars.next() // return next
    }

    fn identifier(&mut self) -> Token {
        while self.peek().is_some_and(is_identifier_char) {
            self.advance();
        }

        let text = &self.source[self.start..self.current];
        let token_type = Self::keyword(text).unwrap_or(TokenType::Identifier);
        self.add_token(token_type)
    }

    fn keyword(text: &str) -> Option<TokenType> {
        KEYWORDS
            .iter()
            .find(|(kw, _)| *kw == text)
            .map(|(_, tt)| *tt)
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
        self.add_token_with_literal(TokenType::Number, Literal::Number(value))
    }

    fn string(&mut self) -> Result<Token, ManoError> {
        let content_start = self.current; // Position after opening quote

        loop {
            match self.peek() {
                None => {
                    return Err(ManoError::Scan {
                        message: "Fechou a string não, maluco!".to_string(),
                        span: self.start..self.current,
                    });
                }
                Some('"') => {
                    // End of string - extract content and consume closing quote
                    let value = self.source[content_start..self.current].to_string();
                    self.advance();
                    return Ok(
                        self.add_token_with_literal(TokenType::String, Literal::String(value))
                    );
                }
                Some('{') => {
                    // Check for escape {{ -> literal {
                    if self.peek_next() == Some('{') {
                        self.advance(); // consume first {
                        self.advance(); // consume second {
                        continue;
                    }
                    // Start of interpolation - emit StringStart
                    let value = self.source[content_start..self.current].to_string();
                    self.advance(); // consume {
                    self.interpolation_stack.push(0);
                    return Ok(Token {
                        token_type: TokenType::StringStart,
                        lexeme: self.source[self.start..self.current].to_string(),
                        literal: Some(Literal::String(value)),
                        span: self.start..self.current,
                    });
                }
                Some('\\') => {
                    // Escape sequence - skip both chars
                    self.advance();
                    if !self.is_at_end() {
                        self.advance();
                    }
                }
                Some(_) => {
                    self.advance();
                }
            }
        }
    }

    /// Continue scanning an interpolated string after a closing }
    fn interpolated_string_continue(&mut self) -> Result<Token, ManoError> {
        let content_start = self.current; // Position after the }
        let token_start = self.start; // Points to the }

        loop {
            match self.peek() {
                None => {
                    return Err(ManoError::Scan {
                        message: "Fechou a string não, maluco!".to_string(),
                        span: token_start..self.current,
                    });
                }
                Some('"') => {
                    // End of interpolated string
                    let value = self.source[content_start..self.current].to_string();
                    self.advance();
                    return Ok(Token {
                        token_type: TokenType::StringEnd,
                        lexeme: self.source[token_start..self.current].to_string(),
                        literal: Some(Literal::String(value)),
                        span: token_start..self.current,
                    });
                }
                Some('{') => {
                    // Check for escape {{ -> literal {
                    if self.peek_next() == Some('{') {
                        self.advance();
                        self.advance();
                        continue;
                    }
                    // Another interpolation - emit StringMiddle
                    let value = self.source[content_start..self.current].to_string();
                    self.advance(); // consume {
                    self.interpolation_stack.push(0);
                    return Ok(Token {
                        token_type: TokenType::StringMiddle,
                        lexeme: self.source[token_start..self.current].to_string(),
                        literal: Some(Literal::String(value)),
                        span: token_start..self.current,
                    });
                }
                Some('\\') => {
                    self.advance();
                    if !self.is_at_end() {
                        self.advance();
                    }
                }
                Some(_) => {
                    self.advance();
                }
            }
        }
    }

    fn block_comment(&mut self) -> Result<(), ManoError> {
        let mut depth = 1;

        while depth > 0 && !self.is_at_end() {
            let c = self.advance();

            if c == '/' && self.peek() == Some('*') {
                self.advance(); // consume '*'
                depth += 1;
            } else if c == '*' && self.peek() == Some('/') {
                self.advance(); // consume '/'
                depth -= 1;
            }
        }

        if depth > 0 {
            return Err(ManoError::Scan {
                message: "Cadê o fecha comentário?".to_string(),
                span: self.start..self.current,
            });
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
    fn tokens_have_correct_spans() {
        let mut scanner = Scanner::new("(\n)");

        let token1 = scanner.next().unwrap().unwrap();
        assert_eq!(token1.span, 0..1); // "(" is at byte 0

        let token2 = scanner.next().unwrap().unwrap();
        assert_eq!(token2.token_type, TokenType::RightParen);
        assert_eq!(token2.span, 2..3); // ")" is at byte 2 (after "(\n")
    }

    #[test]
    fn spans_handle_unicode_correctly() {
        // "sePá" is 5 bytes: s(1) e(1) P(1) á(2)
        // "x" is 1 byte
        // Source: "sePá x" = 7 bytes total (5 + space + 1)
        let source = "sePá x";
        assert_eq!(source.len(), 7); // Verify our byte count

        let mut scanner = Scanner::new(source);

        let keyword = scanner.next().unwrap().unwrap();
        assert_eq!(keyword.token_type, TokenType::If);
        assert_eq!(keyword.lexeme, "sePá");
        assert_eq!(keyword.span, 0..5); // 5 bytes for "sePá"

        let ident = scanner.next().unwrap().unwrap();
        assert_eq!(ident.token_type, TokenType::Identifier);
        assert_eq!(ident.lexeme, "x");
        assert_eq!(ident.span, 6..7); // "x" starts at byte 6

        // Verify we can slice back to the lexeme
        assert_eq!(&source[keyword.span.clone()], "sePá");
        assert_eq!(&source[ident.span.clone()], "x");
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
        if let ManoError::Scan { message, .. } = second.unwrap_err() {
            assert!(message.contains('@'));
        } else {
            panic!("Expected Scan error");
        }

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
    fn scans_percent() {
        let mut scanner = Scanner::new("%");
        let token = scanner.next().unwrap().unwrap();
        assert_eq!(token.token_type, TokenType::Percent);
        assert_eq!(token.lexeme, "%");
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
        assert_eq!(first.span, 0..1);

        let second = scanner.next().unwrap().unwrap();
        assert_eq!(second.token_type, TokenType::RightParen);
        assert_eq!(second.span, 23..24); // ")" at position 23 (len of "( // this is a comment\n" is 24)
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
        use crate::token::Literal;

        let mut scanner = Scanner::new("\"mano\"");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::String);
        assert_eq!(token.lexeme, "\"mano\"");
        assert_eq!(token.literal, Some(Literal::String("mano".to_string())));
    }

    #[test]
    fn scans_string_literal_with_unicode() {
        use crate::token::Literal;

        let mut scanner = Scanner::new("\"e aí mano, beleza?\"");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::String);
        assert_eq!(token.lexeme, "\"e aí mano, beleza?\"");
        assert_eq!(
            token.literal,
            Some(Literal::String("e aí mano, beleza?".to_string()))
        );
    }

    #[test]
    fn unterminated_string_returns_error() {
        let mut scanner = Scanner::new("\"esqueceu de fechar");
        let result = scanner.next().unwrap();

        assert!(result.is_err());
        if let ManoError::Scan { message, .. } = result.unwrap_err() {
            assert!(message.contains("string"));
        } else {
            panic!("Expected Scan error");
        }
    }

    #[test]
    fn unterminated_string_reports_starting_line() {
        // String starts on line 1, spans 3 lines, error should report line 1
        let mut scanner = Scanner::new("\"começou aqui\ne continua\ne nunca fecha");
        let result = scanner.next().unwrap();

        assert!(result.is_err());
        if let ManoError::Scan { message, .. } = result.unwrap_err() {
            assert!(message.contains("string"));
        } else {
            panic!("Expected Scan error");
        }
    }

    #[test]
    fn scans_multiline_string() {
        use crate::token::Literal;

        let source = "\"primeira linha\nsegunda linha\"";
        let mut scanner = Scanner::new(source);
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::String);
        assert_eq!(
            token.literal,
            Some(Literal::String("primeira linha\nsegunda linha".to_string()))
        );
        assert_eq!(token.span, 0..source.len()); // Spans the entire string

        let eof = scanner.next().unwrap().unwrap();
        assert_eq!(eof.span, source.len()..source.len()); // Eof at end
    }

    #[test]
    fn scans_integer_literal() {
        let mut scanner = Scanner::new("1234");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Number);
        assert_eq!(token.lexeme, "1234");
        assert_eq!(token.literal, Some(Literal::Number(1234.0)));
    }

    #[test]
    fn scans_decimal_literal() {
        let mut scanner = Scanner::new("12.34");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Number);
        assert_eq!(token.lexeme, "12.34");
        assert_eq!(token.literal, Some(Literal::Number(12.34)));
    }

    #[test]
    fn trailing_dot_is_not_decimal() {
        // "1234." should be number 1234 followed by dot
        let mut scanner = Scanner::new("1234.");

        let num = scanner.next().unwrap().unwrap();
        assert_eq!(num.token_type, TokenType::Number);
        assert_eq!(num.literal, Some(Literal::Number(1234.0)));

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
        assert_eq!(num.literal, Some(Literal::Number(1234.0)));
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
    fn scans_identifier_with_emoji() {
        let mut scanner = Scanner::new("🔥");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Identifier);
        assert_eq!(token.lexeme, "🔥");
    }

    #[test]
    fn scans_identifier_mixing_emoji_and_text() {
        let mut scanner = Scanner::new("var🚀test");
        let token = scanner.next().unwrap().unwrap();

        assert_eq!(token.token_type, TokenType::Identifier);
        assert_eq!(token.lexeme, "var🚀test");
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
        let source = "(\n/* linha 1\nlinha 2\nlinha 3 */\n)";
        let scanner = Scanner::new(source);
        let tokens: Vec<_> = scanner.collect();

        assert_eq!(tokens.len(), 3);
        // ")" is at the end of the source
        let paren = tokens[1].as_ref().unwrap();
        assert_eq!(paren.token_type, TokenType::RightParen);
        assert_eq!(paren.span, (source.len() - 1)..source.len());
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
        if let Err(ManoError::Scan { message, .. }) = error {
            assert!(message.contains("comentário"));
        } else {
            panic!("Expected Scan error");
        }
    }

    #[test]
    fn with_comments_emits_line_comment_token() {
        let scanner = Scanner::with_comments("// comentário");
        let tokens: Vec<_> = scanner.flatten().collect();

        assert_eq!(tokens.len(), 2); // Comment, Eof
        assert_eq!(tokens[0].token_type, TokenType::Comment);
        assert_eq!(tokens[0].lexeme, "// comentário");
    }

    #[test]
    fn with_comments_emits_block_comment_token() {
        let scanner = Scanner::with_comments("/* bloco */");
        let tokens: Vec<_> = scanner.flatten().collect();

        assert_eq!(tokens.len(), 2); // Comment, Eof
        assert_eq!(tokens[0].token_type, TokenType::Comment);
        assert_eq!(tokens[0].lexeme, "/* bloco */");
    }

    #[test]
    fn with_comments_includes_inline_comments() {
        let scanner = Scanner::with_comments("salve /* inline */ 42");
        let tokens: Vec<_> = scanner.flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::Print); // salve
        assert_eq!(tokens[1].token_type, TokenType::Comment); // /* inline */
        assert_eq!(tokens[2].token_type, TokenType::Number); // 42
    }

    #[test]
    fn with_comments_emits_multiline_block_comment() {
        let scanner = Scanner::with_comments("/* linha 1\nlinha 2 */");
        let tokens: Vec<_> = scanner.flatten().collect();

        assert_eq!(tokens.len(), 2); // Comment, Eof
        assert_eq!(tokens[0].token_type, TokenType::Comment);
        assert_eq!(tokens[0].lexeme, "/* linha 1\nlinha 2 */");
    }

    #[test]
    fn without_comments_skips_comments() {
        let scanner = Scanner::new("salve /* inline */ 42");
        let tokens: Vec<_> = scanner.flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::Print); // salve
        assert_eq!(tokens[1].token_type, TokenType::Number); // 42
        assert_eq!(tokens[2].token_type, TokenType::Eof);
    }

    #[test]
    fn is_identifier_start_accepts_letters_underscore_emoji() {
        assert!(is_identifier_start('a'));
        assert!(is_identifier_start('Z'));
        assert!(is_identifier_start('_'));
        assert!(is_identifier_start('é'));
        assert!(is_identifier_start('🔥'));

        assert!(!is_identifier_start('0'));
        assert!(!is_identifier_start(' '));
        assert!(!is_identifier_start('+'));
    }

    #[test]
    fn is_identifier_char_accepts_letters_digits_underscore_emoji() {
        assert!(is_identifier_char('a'));
        assert!(is_identifier_char('Z'));
        assert!(is_identifier_char('_'));
        assert!(is_identifier_char('0'));
        assert!(is_identifier_char('9'));
        assert!(is_identifier_char('é'));
        assert!(is_identifier_char('🔥'));

        assert!(!is_identifier_char(' '));
        assert!(!is_identifier_char('+'));
    }

    // === String interpolation tests ===

    #[test]
    fn scans_interpolated_string_simple() {
        let mut scanner = Scanner::new("\"Olá, {nome}!\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::StringStart);
        assert_eq!(
            tokens[0].literal,
            Some(Literal::String("Olá, ".to_string()))
        );

        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[1].lexeme, "nome");

        assert_eq!(tokens[2].token_type, TokenType::StringEnd);
        assert_eq!(tokens[2].literal, Some(Literal::String("!".to_string())));

        assert_eq!(tokens[3].token_type, TokenType::Eof);
    }

    #[test]
    fn scans_interpolated_string_multiple() {
        let mut scanner = Scanner::new("\"Olá, {nome}! Você tem {idade} anos.\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::StringStart);
        assert_eq!(
            tokens[0].literal,
            Some(Literal::String("Olá, ".to_string()))
        );

        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[1].lexeme, "nome");

        assert_eq!(tokens[2].token_type, TokenType::StringMiddle);
        assert_eq!(
            tokens[2].literal,
            Some(Literal::String("! Você tem ".to_string()))
        );

        assert_eq!(tokens[3].token_type, TokenType::Identifier);
        assert_eq!(tokens[3].lexeme, "idade");

        assert_eq!(tokens[4].token_type, TokenType::StringEnd);
        assert_eq!(
            tokens[4].literal,
            Some(Literal::String(" anos.".to_string()))
        );
    }

    #[test]
    fn scans_interpolated_string_with_expression() {
        let mut scanner = Scanner::new("\"Dobro: {x * 2}\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::StringStart);
        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[1].lexeme, "x");
        assert_eq!(tokens[2].token_type, TokenType::Star);
        assert_eq!(tokens[3].token_type, TokenType::Number);
        assert_eq!(tokens[4].token_type, TokenType::StringEnd);
    }

    #[test]
    fn scans_plain_string_without_interpolation() {
        let mut scanner = Scanner::new("\"Hello world\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::String);
        assert_eq!(
            tokens[0].literal,
            Some(Literal::String("Hello world".to_string()))
        );
    }

    #[test]
    fn scans_escaped_brace_not_interpolation() {
        let mut scanner = Scanner::new("\"Literal {{braces}}\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        // Should be a plain string, not interpolation
        assert_eq!(tokens[0].token_type, TokenType::String);
        assert!(
            tokens[0]
                .literal
                .as_ref()
                .unwrap()
                .to_string()
                .contains("{")
        );
    }

    #[test]
    fn scans_interpolation_with_nested_braces() {
        // Code inside interpolation can have its own braces
        let mut scanner = Scanner::new("\"Result: {foo({x})}\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::StringStart);
        assert_eq!(tokens[1].token_type, TokenType::Identifier); // foo
        assert_eq!(tokens[2].token_type, TokenType::LeftParen);
        assert_eq!(tokens[3].token_type, TokenType::LeftBrace);
        assert_eq!(tokens[4].token_type, TokenType::Identifier); // x
        assert_eq!(tokens[5].token_type, TokenType::RightBrace);
        assert_eq!(tokens[6].token_type, TokenType::RightParen);
        assert_eq!(tokens[7].token_type, TokenType::StringEnd);
    }

    #[test]
    fn scans_escape_in_interpolation_start() {
        // Escape sequence in the first part of interpolated string
        let mut scanner = Scanner::new("\"Hello\\n{x}\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::StringStart);
        assert!(tokens[0].lexeme.contains("\\n"));
        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[2].token_type, TokenType::StringEnd);
    }

    #[test]
    fn scans_escape_in_interpolation_continue() {
        // Escape sequence after an interpolation
        let mut scanner = Scanner::new("\"{x}\\nWorld\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::StringStart);
        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[2].token_type, TokenType::StringEnd);
        assert!(tokens[2].lexeme.contains("\\n"));
    }

    #[test]
    fn scans_escaped_brace_in_interpolation_continue() {
        // {{ escape in the middle/end part of interpolated string
        let mut scanner = Scanner::new("\"{x}{{literal}}\"");
        let tokens: Vec<_> = scanner.by_ref().flatten().collect();

        assert_eq!(tokens[0].token_type, TokenType::StringStart);
        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[2].token_type, TokenType::StringEnd);
        // The {{ should remain as {{ (not converted, just not starting interpolation)
        assert!(tokens[2].lexeme.contains("{"));
    }

    #[test]
    fn unterminated_interpolated_string_returns_error() {
        // String that starts interpolation but never closes
        let scanner = Scanner::new("\"{x}");
        let results: Vec<_> = scanner.collect();

        // Should have StringStart, Identifier, then an error
        assert!(results.iter().any(|r| r.is_err()));
    }
}
