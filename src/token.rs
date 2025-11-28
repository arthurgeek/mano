#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Single-character tokens
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,

    // One or two character tokens
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals
    String,
    Number,
    Identifier,

    // Keywords
    And,    // tamoJunto
    Class,  // bagulho
    Else,   // vacilou
    False,  // treta
    Fun,    // olhaEssaFita
    For,    // seVira
    If,     // sePá
    Nil,    // nadaNão
    Or,     // ow
    Print,  // salve, oiSumida
    Return, // toma
    Super,  // mestre
    This,   // oCara
    True,   // firmeza
    Var,    // seLiga
    While,  // segueOFluxo
    Break,  // saiFora

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
}

#[derive(Debug)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub literal: Option<Value>,
    #[allow(dead_code)] // Used later for error reporting
    pub line: usize,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.literal {
            Some(value) => write!(f, "{:?} {} {}", self.token_type, self.lexeme, value),
            None => write!(f, "{:?} {} None", self.token_type, self.lexeme),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_display_without_literal() {
        let token = Token {
            token_type: TokenType::LeftParen,
            lexeme: "(".to_string(),
            literal: None,
            line: 1,
        };
        assert_eq!(token.to_string(), "LeftParen ( None");
    }

    #[test]
    fn token_display_with_number() {
        let token = Token {
            token_type: TokenType::LeftParen,
            lexeme: "42".to_string(),
            literal: Some(Value::Number(42.0)),
            line: 1,
        };
        assert_eq!(token.to_string(), "LeftParen 42 42");
    }
}
