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
    Variable {
        name: Token,
    },
    Assign {
        name: Token,
        value: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Expression {
        expression: Expr,
    },
    Print {
        expression: Expr,
    },
    Var {
        name: Token,
        initializer: Option<Expr>,
    },
    Block {
        statements: Vec<Stmt>,
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
            Expr::Variable { name } => write!(f, "{}", name.lexeme),
            Expr::Assign { name, value } => write!(f, "(= {} {})", name.lexeme, value),
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
            span: 0..lexeme.len(),
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
    fn creates_variable_expression() {
        let expr = Expr::Variable {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "x".to_string(),
                literal: None,
                span: 0..1,
            },
        };
        assert!(matches!(expr, Expr::Variable { name } if name.lexeme == "x"));
    }

    #[test]
    fn displays_variable_expression() {
        let expr = Expr::Variable {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "meuMano".to_string(),
                literal: None,
                span: 0..7,
            },
        };
        assert_eq!(expr.to_string(), "meuMano");
    }

    #[test]
    fn creates_assign_expression() {
        let expr = Expr::Assign {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "x".to_string(),
                literal: None,
                span: 0..1,
            },
            value: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
        };
        assert!(matches!(expr, Expr::Assign { name, .. } if name.lexeme == "x"));
    }

    #[test]
    fn displays_assign_expression() {
        let expr = Expr::Assign {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "x".to_string(),
                literal: None,
                span: 0..1,
            },
            value: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
        };
        assert_eq!(expr.to_string(), "(= x 42)");
    }

    #[test]
    fn creates_block_statement() {
        let stmt = Stmt::Block {
            statements: vec![Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(42.0),
                },
            }],
        };
        assert!(matches!(stmt, Stmt::Block { statements } if statements.len() == 1));
    }

    #[test]
    fn creates_empty_block_statement() {
        let stmt = Stmt::Block { statements: vec![] };
        assert!(matches!(stmt, Stmt::Block { statements } if statements.is_empty()));
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
