use std::fmt;

use crate::token::{Token, Value};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Binary {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
    },
    Ternary {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Unary {
        operator: Token,
        right: Box<Expr>,
    },
    Literal {
        value: Value,
    },
    Grouping {
        expression: Box<Expr>,
    },
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Binary {
                left,
                operator,
                right,
            } => write!(f, "({} {} {})", operator.lexeme, left, right),
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => write!(f, "(?: {} {} {})", condition, then_branch, else_branch),
            Expr::Unary { operator, right } => write!(f, "({} {})", operator.lexeme, right),
            Expr::Literal { value } => write!(f, "{}", value),
            Expr::Grouping { expression } => write!(f, "(group {})", expression),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenType;

    fn make_token(token_type: TokenType, lexeme: &str) -> Token {
        Token {
            token_type,
            lexeme: lexeme.to_string(),
            literal: None,
            line: 1,
        }
    }

    #[test]
    fn creates_literal_number() {
        let expr = Expr::Literal {
            value: Value::Number(42.0),
        };
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Value::Number(n)
            } if n == 42.0
        ));
    }

    #[test]
    fn creates_literal_string() {
        let expr = Expr::Literal {
            value: Value::String("mano".to_string()),
        };
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Value::String(ref s)
            } if s == "mano"
        ));
    }

    #[test]
    fn creates_literal_bool() {
        let expr = Expr::Literal {
            value: Value::Bool(true),
        };
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Value::Bool(true)
            }
        ));
    }

    #[test]
    fn creates_literal_nil() {
        let expr = Expr::Literal { value: Value::Nil };
        assert!(matches!(expr, Expr::Literal { value: Value::Nil }));
    }

    #[test]
    fn creates_unary_expression() {
        let expr = Expr::Unary {
            operator: make_token(TokenType::Minus, "-"),
            right: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
        };
        assert!(matches!(expr, Expr::Unary { .. }));
    }

    #[test]
    fn creates_binary_expression() {
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(1.0),
            }),
            operator: make_token(TokenType::Plus, "+"),
            right: Box::new(Expr::Literal {
                value: Value::Number(2.0),
            }),
        };
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    #[test]
    fn creates_grouping_expression() {
        let expr = Expr::Grouping {
            expression: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
        };
        assert!(matches!(expr, Expr::Grouping { .. }));
    }

    #[test]
    fn creates_nested_expression() {
        // Represents: -(-5)
        let expr = Expr::Unary {
            operator: make_token(TokenType::Minus, "-"),
            right: Box::new(Expr::Unary {
                operator: make_token(TokenType::Minus, "-"),
                right: Box::new(Expr::Literal {
                    value: Value::Number(5.0),
                }),
            }),
        };
        assert!(matches!(expr, Expr::Unary { .. }));
    }

    #[test]
    fn displays_nested_expression() {
        // -123 * (45.67)
        let expr = Expr::Binary {
            left: Box::new(Expr::Unary {
                operator: make_token(TokenType::Minus, "-"),
                right: Box::new(Expr::Literal {
                    value: Value::Number(123.0),
                }),
            }),
            operator: make_token(TokenType::Star, "*"),
            right: Box::new(Expr::Grouping {
                expression: Box::new(Expr::Literal {
                    value: Value::Number(45.67),
                }),
            }),
        };

        assert_eq!(expr.to_string(), "(* (- 123) (group 45.67))");
    }

    #[test]
    fn displays_ternary_expression() {
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Literal {
                value: Value::Bool(true),
            }),
            then_branch: Box::new(Expr::Literal {
                value: Value::Number(1.0),
            }),
            else_branch: Box::new(Expr::Literal {
                value: Value::Number(2.0),
            }),
        };

        assert_eq!(expr.to_string(), "(?: firmeza 1 2)");
    }
}
