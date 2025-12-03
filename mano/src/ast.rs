use std::fmt;

use crate::token::{Literal, Token};

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
        value: Literal,
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
    Logical {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        paren: Token,
        arguments: Vec<Expr>,
    },
    Lambda {
        params: Vec<Token>,
        body: Vec<Stmt>,
    },
    Get {
        object: Box<Expr>,
        name: Token,
    },
    Set {
        object: Box<Expr>,
        name: Token,
        value: Box<Expr>,
    },
    This {
        keyword: Token,
    },
}

pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Expression {
        expression: Expr,
        span: Span,
    },
    Print {
        expression: Expr,
        span: Span,
    },
    Var {
        name: Token,
        initializer: Option<Expr>,
        span: Span,
    },
    Block {
        statements: Vec<Stmt>,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Else {
        body: Box<Stmt>,
        span: Span,
    },
    Function {
        name: Token,
        params: Vec<Token>,
        body: Vec<Stmt>,
        is_static: bool,
        is_getter: bool,
        span: Span,
    },
    Return {
        keyword: Token,
        value: Option<Expr>,
        span: Span,
    },
    Class {
        name: Token,
        methods: Vec<Stmt>,
        span: Span,
    },
}

impl Stmt {
    pub fn children(&self) -> Vec<&Stmt> {
        match self {
            Stmt::Block { statements, .. } => statements.iter().collect(),
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                let mut children = vec![then_branch.as_ref()];
                if let Some(eb) = else_branch {
                    children.push(eb.as_ref());
                }
                children
            }
            Stmt::While { body, .. } | Stmt::Else { body, .. } => vec![body.as_ref()],
            _ => vec![],
        }
    }

    pub fn var_declaration(&self) -> Option<(&Token, &Option<Expr>)> {
        match self {
            Stmt::Var {
                name, initializer, ..
            } => Some((name, initializer)),
            _ => None,
        }
    }

    pub fn function_declaration(&self) -> Option<(&Token, &Vec<Token>, &Vec<Stmt>)> {
        match self {
            Stmt::Function {
                name, params, body, ..
            } => Some((name, params, body)),
            _ => None,
        }
    }

    /// Returns (name, methods) if this is a class declaration
    pub fn class_declaration(&self) -> Option<(&Token, &Vec<Stmt>)> {
        match self {
            Stmt::Class { name, methods, .. } => Some((name, methods)),
            _ => None,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Stmt::Expression { span, .. }
            | Stmt::Print { span, .. }
            | Stmt::Var { span, .. }
            | Stmt::Block { span, .. }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::Break { span, .. }
            | Stmt::Else { span, .. }
            | Stmt::Function { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Class { span, .. } => span.clone(),
        }
    }
}

/// Helper constructors for tests that don't need real spans
#[cfg(test)]
impl Stmt {
    pub fn print(expression: Expr) -> Self {
        Stmt::Print {
            expression,
            span: 0..0,
        }
    }

    pub fn expression(expression: Expr) -> Self {
        Stmt::Expression {
            expression,
            span: 0..0,
        }
    }

    pub fn var(name: Token, initializer: Option<Expr>) -> Self {
        Stmt::Var {
            name,
            initializer,
            span: 0..0,
        }
    }

    pub fn block(statements: Vec<Stmt>) -> Self {
        Stmt::Block {
            statements,
            span: 0..0,
        }
    }

    pub fn if_stmt(condition: Expr, then_branch: Stmt, else_branch: Option<Stmt>) -> Self {
        Stmt::If {
            condition,
            then_branch: Box::new(then_branch),
            else_branch: else_branch.map(Box::new),
            span: 0..0,
        }
    }

    pub fn while_stmt(condition: Expr, body: Stmt) -> Self {
        Stmt::While {
            condition,
            body: Box::new(body),
            span: 0..0,
        }
    }

    pub fn break_stmt() -> Self {
        Stmt::Break { span: 0..0 }
    }
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
            Expr::Logical {
                left,
                operator,
                right,
            } => write!(f, "({} {} {})", operator.lexeme, left, right),
            Expr::Call {
                callee, arguments, ..
            } => {
                write!(f, "(call {}", callee)?;
                for arg in arguments {
                    write!(f, " {}", arg)?;
                }
                write!(f, ")")
            }
            Expr::Lambda { params, .. } => {
                write!(f, "(lambda")?;
                for param in params {
                    write!(f, " {}", param.lexeme)?;
                }
                write!(f, ")")
            }
            Expr::Get { object, name } => write!(f, "{}.{}", object, name.lexeme),
            Expr::Set {
                object,
                name,
                value,
            } => write!(f, "({}.{} = {})", object, name.lexeme, value),
            Expr::This { .. } => write!(f, "oCara"),
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
            value: Literal::Number(42.0),
        };
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Literal::Number(n)
            } if n == 42.0
        ));
    }

    #[test]
    fn creates_literal_string() {
        let expr = Expr::Literal {
            value: Literal::String("mano".to_string()),
        };
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Literal::String(ref s)
            } if s == "mano"
        ));
    }

    #[test]
    fn creates_literal_bool() {
        let expr = Expr::Literal {
            value: Literal::Bool(true),
        };
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Literal::Bool(true)
            }
        ));
    }

    #[test]
    fn creates_literal_nil() {
        let expr = Expr::Literal {
            value: Literal::Nil,
        };
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Literal::Nil
            }
        ));
    }

    #[test]
    fn creates_unary_expression() {
        let expr = Expr::Unary {
            operator: make_token(TokenType::Minus, "-"),
            right: Box::new(Expr::Literal {
                value: Literal::Number(5.0),
            }),
        };
        assert!(matches!(expr, Expr::Unary { .. }));
    }

    #[test]
    fn creates_binary_expression() {
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            operator: make_token(TokenType::Plus, "+"),
            right: Box::new(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        };
        assert!(matches!(expr, Expr::Binary { .. }));
    }

    #[test]
    fn creates_grouping_expression() {
        let expr = Expr::Grouping {
            expression: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
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
                value: Literal::Number(42.0),
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
                value: Literal::Number(42.0),
            }),
        };
        assert_eq!(expr.to_string(), "(= x 42)");
    }

    #[test]
    fn creates_block_statement() {
        let stmt = Stmt::block(vec![Stmt::print(Expr::Literal {
            value: Literal::Number(42.0),
        })]);
        assert!(matches!(stmt, Stmt::Block { statements, .. } if statements.len() == 1));
    }

    #[test]
    fn creates_empty_block_statement() {
        let stmt = Stmt::block(vec![]);
        assert!(matches!(stmt, Stmt::Block { statements, .. } if statements.is_empty()));
    }

    #[test]
    fn creates_nested_expression() {
        // Represents: -(-5)
        let expr = Expr::Unary {
            operator: make_token(TokenType::Minus, "-"),
            right: Box::new(Expr::Unary {
                operator: make_token(TokenType::Minus, "-"),
                right: Box::new(Expr::Literal {
                    value: Literal::Number(5.0),
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
                    value: Literal::Number(123.0),
                }),
            }),
            operator: make_token(TokenType::Star, "*"),
            right: Box::new(Expr::Grouping {
                expression: Box::new(Expr::Literal {
                    value: Literal::Number(45.67),
                }),
            }),
        };

        assert_eq!(expr.to_string(), "(* (- 123) (group 45.67))");
    }

    #[test]
    fn displays_ternary_expression() {
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Literal {
                value: Literal::Bool(true),
            }),
            then_branch: Box::new(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            else_branch: Box::new(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        };

        assert_eq!(expr.to_string(), "(?: firmeza 1 2)");
    }

    #[test]
    fn stmt_span_returns_span_for_each_variant() {
        let span = 10..20;

        let expr = Stmt::Expression {
            expression: Expr::Literal {
                value: Literal::Number(1.0),
            },
            span: span.clone(),
        };
        assert_eq!(expr.span(), span);

        let print = Stmt::Print {
            expression: Expr::Literal {
                value: Literal::Number(1.0),
            },
            span: span.clone(),
        };
        assert_eq!(print.span(), span);

        let var = Stmt::Var {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "x".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: None,
            span: 30..40,
        };
        assert_eq!(var.span(), 30..40);

        let block = Stmt::Block {
            statements: vec![],
            span: 5..15,
        };
        assert_eq!(block.span(), 5..15);

        let if_stmt = Stmt::If {
            condition: Expr::Literal {
                value: Literal::Bool(true),
            },
            then_branch: Box::new(Stmt::break_stmt()),
            else_branch: None,
            span: 50..60,
        };
        assert_eq!(if_stmt.span(), 50..60);

        let while_stmt = Stmt::While {
            condition: Expr::Literal {
                value: Literal::Bool(true),
            },
            body: Box::new(Stmt::break_stmt()),
            span: 70..80,
        };
        assert_eq!(while_stmt.span(), 70..80);

        let break_stmt = Stmt::Break { span: 90..95 };
        assert_eq!(break_stmt.span(), 90..95);

        let else_stmt = Stmt::Else {
            body: Box::new(Stmt::print(Expr::Literal {
                value: Literal::Nil,
            })),
            span: 100..200,
        };
        assert_eq!(else_stmt.span(), 100..200);
    }

    #[test]
    fn stmt_children_returns_empty_for_simple_statements() {
        let print = Stmt::print(Expr::Literal {
            value: Literal::Nil,
        });
        assert!(print.children().is_empty());

        let break_stmt = Stmt::break_stmt();
        assert!(break_stmt.children().is_empty());
    }

    #[test]
    fn stmt_children_returns_statements_for_block() {
        let inner1 = Stmt::Print {
            expression: Expr::Literal {
                value: Literal::Number(1.0),
            },
            span: 10..20,
        };
        let inner2 = Stmt::Print {
            expression: Expr::Literal {
                value: Literal::Number(2.0),
            },
            span: 30..40,
        };
        let block = Stmt::block(vec![inner1, inner2]);

        let children = block.children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].span(), 10..20);
        assert_eq!(children[1].span(), 30..40);
    }

    #[test]
    fn stmt_children_returns_branches_for_if_else() {
        let then_stmt = Stmt::Print {
            expression: Expr::Literal {
                value: Literal::Number(1.0),
            },
            span: 100..110,
        };
        let else_stmt = Stmt::Print {
            expression: Expr::Literal {
                value: Literal::Number(2.0),
            },
            span: 200..210,
        };
        let if_stmt = Stmt::if_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            then_stmt,
            Some(else_stmt),
        );

        let children = if_stmt.children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].span(), 100..110);
        assert_eq!(children[1].span(), 200..210);
    }

    #[test]
    fn stmt_children_returns_body_for_while() {
        let body = Stmt::Print {
            expression: Expr::Literal {
                value: Literal::Number(42.0),
            },
            span: 50..60,
        };
        let while_stmt = Stmt::while_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            body,
        );

        let children = while_stmt.children();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].span(), 50..60);
    }

    #[test]
    fn stmt_var_declaration_returns_name_and_initializer() {
        let name = Token {
            token_type: TokenType::Identifier,
            lexeme: "meuMano".to_string(),
            literal: None,
            span: 7..14,
        };
        let init = Expr::Literal {
            value: Literal::Number(42.0),
        };
        let var = Stmt::Var {
            name: name.clone(),
            initializer: Some(init),
            span: 0..18,
        };

        let (n, i) = var.var_declaration().expect("should return Some for Var");
        assert_eq!(n.lexeme, "meuMano");
        assert_eq!(n.span, 7..14);
        assert!(matches!(
            i,
            Some(Expr::Literal { value: Literal::Number(n) }) if *n == 42.0
        ));
    }

    #[test]
    fn stmt_var_declaration_returns_none_for_expression() {
        let stmt = Stmt::expression(Expr::Literal {
            value: Literal::Nil,
        });
        assert!(stmt.var_declaration().is_none());
    }

    #[test]
    fn stmt_var_declaration_returns_none_for_print() {
        let stmt = Stmt::print(Expr::Literal {
            value: Literal::Nil,
        });
        assert!(stmt.var_declaration().is_none());
    }

    #[test]
    fn stmt_var_declaration_returns_none_for_block() {
        let stmt = Stmt::block(vec![]);
        assert!(stmt.var_declaration().is_none());
    }

    #[test]
    fn stmt_var_declaration_returns_none_for_if() {
        let stmt = Stmt::if_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            Stmt::break_stmt(),
            None,
        );
        assert!(stmt.var_declaration().is_none());
    }

    #[test]
    fn stmt_var_declaration_returns_none_for_while() {
        let stmt = Stmt::while_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            Stmt::break_stmt(),
        );
        assert!(stmt.var_declaration().is_none());
    }

    #[test]
    fn stmt_var_declaration_returns_none_for_break() {
        let stmt = Stmt::break_stmt();
        assert!(stmt.var_declaration().is_none());
    }

    #[test]
    fn stmt_var_declaration_returns_none_for_else() {
        let stmt = Stmt::Else {
            body: Box::new(Stmt::break_stmt()),
            span: 0..10,
        };
        assert!(stmt.var_declaration().is_none());
    }

    #[test]
    fn displays_call_with_no_arguments() {
        let callee = Expr::Variable {
            name: make_token(TokenType::Identifier, "fazTeuCorre"),
        };
        let expr = Expr::Call {
            callee: Box::new(callee),
            paren: make_token(TokenType::RightParen, ")"),
            arguments: vec![],
        };
        assert_eq!(expr.to_string(), "(call fazTeuCorre)");
    }

    #[test]
    fn displays_call_with_arguments() {
        let callee = Expr::Variable {
            name: make_token(TokenType::Identifier, "soma"),
        };
        let expr = Expr::Call {
            callee: Box::new(callee),
            paren: make_token(TokenType::RightParen, ")"),
            arguments: vec![
                Expr::Literal {
                    value: Literal::Number(1.0),
                },
                Expr::Literal {
                    value: Literal::Number(2.0),
                },
            ],
        };
        assert_eq!(expr.to_string(), "(call soma 1 2)");
    }

    #[test]
    fn creates_function_statement() {
        let stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "cumprimentar"),
            params: vec![make_token(TokenType::Identifier, "nome")],
            body: vec![],
            is_static: false,
            is_getter: false,
            span: 0..30,
        };
        assert!(matches!(stmt, Stmt::Function { params, .. } if params.len() == 1));
    }

    #[test]
    fn stmt_function_declaration_returns_name_params_body() {
        let name = make_token(TokenType::Identifier, "soma");
        let params = vec![
            make_token(TokenType::Identifier, "a"),
            make_token(TokenType::Identifier, "b"),
        ];
        let body = vec![Stmt::Return {
            keyword: make_token(TokenType::Return, "toma"),
            value: None,
            span: 0..5,
        }];
        let stmt = Stmt::Function {
            name: name.clone(),
            params: params.clone(),
            body: body.clone(),
            is_static: false,
            is_getter: false,
            span: 0..30,
        };

        let (n, p, b) = stmt
            .function_declaration()
            .expect("should return Some for Function");
        assert_eq!(n.lexeme, "soma");
        assert_eq!(p.len(), 2);
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn stmt_function_declaration_returns_none_for_var() {
        let stmt = Stmt::var(make_token(TokenType::Identifier, "x"), None);
        assert!(stmt.function_declaration().is_none());
    }

    #[test]
    fn stmt_function_declaration_returns_none_for_print() {
        let stmt = Stmt::print(Expr::Literal {
            value: Literal::Nil,
        });
        assert!(stmt.function_declaration().is_none());
    }

    #[test]
    fn stmt_function_declaration_returns_none_for_block() {
        let stmt = Stmt::block(vec![]);
        assert!(stmt.function_declaration().is_none());
    }

    #[test]
    fn stmt_class_declaration_returns_name_and_methods() {
        let name = make_token(TokenType::Identifier, "Pessoa");
        let method = Stmt::Function {
            name: make_token(TokenType::Identifier, "falar"),
            params: vec![],
            body: vec![],
            is_static: false,
            is_getter: false,
            span: 10..20,
        };
        let stmt = Stmt::Class {
            name: name.clone(),
            methods: vec![method],
            span: 0..30,
        };
        let (decl_name, methods) = stmt.class_declaration().unwrap();
        assert_eq!(decl_name.lexeme, "Pessoa");
        assert_eq!(methods.len(), 1);
    }

    #[test]
    fn stmt_class_declaration_returns_none_for_function() {
        let stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "foo"),
            params: vec![],
            body: vec![],
            is_static: false,
            is_getter: false,
            span: 0..10,
        };
        assert!(stmt.class_declaration().is_none());
    }

    #[test]
    fn stmt_class_declaration_returns_none_for_var() {
        let stmt = Stmt::var(make_token(TokenType::Identifier, "x"), None);
        assert!(stmt.class_declaration().is_none());
    }

    #[test]
    fn stmt_span_returns_span_for_function() {
        let stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "foo"),
            params: vec![],
            body: vec![],
            is_static: false,
            is_getter: false,
            span: 10..50,
        };
        assert_eq!(stmt.span(), 10..50);
    }

    #[test]
    fn stmt_span_returns_span_for_return() {
        let stmt = Stmt::Return {
            keyword: make_token(TokenType::Return, "toma"),
            value: None,
            span: 20..25,
        };
        assert_eq!(stmt.span(), 20..25);
    }

    #[test]
    fn displays_lambda_with_params() {
        let expr = Expr::Lambda {
            params: vec![
                make_token(TokenType::Identifier, "a"),
                make_token(TokenType::Identifier, "b"),
            ],
            body: vec![],
        };
        assert_eq!(expr.to_string(), "(lambda a b)");
    }

    #[test]
    fn displays_lambda_with_no_params() {
        let expr = Expr::Lambda {
            params: vec![],
            body: vec![],
        };
        assert_eq!(expr.to_string(), "(lambda)");
    }

    #[test]
    fn creates_class_statement() {
        let name = make_token(TokenType::Identifier, "Pessoa");
        let class = Stmt::Class {
            name: name.clone(),
            methods: vec![],
            span: 0..15,
        };

        assert_eq!(class.span(), 0..15);
    }

    #[test]
    fn class_statement_with_methods() {
        let name = make_token(TokenType::Identifier, "Pessoa");
        let method = Stmt::Function {
            name: make_token(TokenType::Identifier, "falar"),
            params: vec![],
            body: vec![],
            is_static: false,
            is_getter: false,
            span: 20..30,
        };
        let class = Stmt::Class {
            name,
            methods: vec![method],
            span: 0..40,
        };

        assert_eq!(class.span(), 0..40);
        // children() returns empty for Class as methods aren't traversed
        assert!(class.children().is_empty());
    }

    #[test]
    fn creates_get_expression() {
        let expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "pessoa"),
            }),
            name: make_token(TokenType::Identifier, "nome"),
        };
        assert!(matches!(expr, Expr::Get { .. }));
    }

    #[test]
    fn displays_get_expression() {
        let expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "pessoa"),
            }),
            name: make_token(TokenType::Identifier, "nome"),
        };
        assert_eq!(expr.to_string(), "pessoa.nome");
    }

    #[test]
    fn creates_set_expression() {
        let expr = Expr::Set {
            object: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "pessoa"),
            }),
            name: make_token(TokenType::Identifier, "nome"),
            value: Box::new(Expr::Literal {
                value: Literal::String("João".to_string()),
            }),
        };
        assert!(matches!(expr, Expr::Set { .. }));
    }

    #[test]
    fn displays_set_expression() {
        let expr = Expr::Set {
            object: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "pessoa"),
            }),
            name: make_token(TokenType::Identifier, "nome"),
            value: Box::new(Expr::Literal {
                value: Literal::String("João".to_string()),
            }),
        };
        assert_eq!(expr.to_string(), "(pessoa.nome = João)");
    }

    #[test]
    fn displays_this_expression() {
        let expr = Expr::This {
            keyword: make_token(TokenType::This, "oCara"),
        };
        assert_eq!(expr.to_string(), "oCara");
    }
}
