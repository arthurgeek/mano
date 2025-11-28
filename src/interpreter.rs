use crate::ast::Expr;
use crate::error::ManoError;
use crate::token::{TokenType, Value};

pub struct Interpreter;

impl Interpreter {
    pub fn new() -> Self {
        Self
    }

    pub fn interpret(&mut self, expr: &Expr) -> Result<Value, ManoError> {
        match expr {
            Expr::Literal { value } => Ok(value.clone()),
            Expr::Grouping { expression } => self.interpret(expression),
            Expr::Unary { operator, right } => {
                let right_val = self.interpret(right)?;
                match operator.token_type {
                    TokenType::Minus => match right_val {
                        Value::Number(n) => Ok(Value::Number(-n)),
                        _ => Err(ManoError::Runtime {
                            line: operator.line,
                            message: "Só dá pra negar número, tio!".to_string(),
                        }),
                    },
                    TokenType::Bang => Ok(Value::Bool(!self.is_truthy(&right_val))),
                    _ => unreachable!(),
                }
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let left_val = self.interpret(left)?;
                let right_val = self.interpret(right)?;

                match operator.token_type {
                    TokenType::Minus | TokenType::Slash | TokenType::Star => {
                        let (a, b) = self.require_numbers(&left_val, &right_val, operator.line)?;
                        match operator.token_type {
                            TokenType::Minus => Ok(Value::Number(a - b)),
                            TokenType::Slash => Ok(Value::Number(a / b)),
                            TokenType::Star => Ok(Value::Number(a * b)),
                            _ => unreachable!(),
                        }
                    }
                    TokenType::Plus => match (&left_val, &right_val) {
                        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                        (Value::String(a), Value::String(b)) => {
                            Ok(Value::String(format!("{}{}", a, b)))
                        }
                        _ => Err(ManoError::Runtime {
                            line: operator.line,
                            message: "Só dá pra somar número com número ou texto com texto, chapa!"
                                .to_string(),
                        }),
                    },
                    TokenType::Greater
                    | TokenType::GreaterEqual
                    | TokenType::Less
                    | TokenType::LessEqual => {
                        let (a, b) = self.require_numbers(&left_val, &right_val, operator.line)?;
                        let result = match operator.token_type {
                            TokenType::Greater => a > b,
                            TokenType::GreaterEqual => a >= b,
                            TokenType::Less => a < b,
                            TokenType::LessEqual => a <= b,
                            _ => unreachable!(),
                        };
                        Ok(Value::Bool(result))
                    }
                    TokenType::EqualEqual => Ok(Value::Bool(self.is_equal(&left_val, &right_val))),
                    TokenType::BangEqual => Ok(Value::Bool(!self.is_equal(&left_val, &right_val))),
                    TokenType::Comma => Ok(right_val),
                    _ => unreachable!(),
                }
            }
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.interpret(condition)?;
                if self.is_truthy(&cond_val) {
                    self.interpret(then_branch)
                } else {
                    self.interpret(else_branch)
                }
            }
        }
    }

    fn is_truthy(&self, value: &Value) -> bool {
        match value {
            Value::Nil => false,
            Value::Bool(b) => *b,
            _ => true,
        }
    }

    fn require_numbers(
        &self,
        left: &Value,
        right: &Value,
        line: usize,
    ) -> Result<(f64, f64), ManoError> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => Ok((*a, *b)),
            _ => Err(ManoError::Runtime {
                line,
                message: "Os dois lados precisam ser número, irmão!".to_string(),
            }),
        }
    }

    fn is_equal(&self, a: &Value, b: &Value) -> bool {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === literals ===

    #[test]
    fn evaluates_number_literal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Value::Number(42.0),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn evaluates_string_literal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Value::String("mano".to_string()),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::String("mano".to_string()));
    }

    #[test]
    fn evaluates_bool_true() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Value::Bool(true),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_bool_false() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Value::Bool(false),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn evaluates_nil() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal { value: Value::Nil };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Nil);
    }

    // === grouping ===

    #[test]
    fn evaluates_grouping() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Grouping {
            expression: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    // === unary ===

    fn make_token(
        token_type: crate::token::TokenType,
        lexeme: &str,
        line: usize,
    ) -> crate::token::Token {
        crate::token::Token {
            token_type,
            lexeme: lexeme.to_string(),
            literal: None,
            line,
        }
    }

    #[test]
    fn evaluates_unary_minus() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Minus, "-", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(-5.0));
    }

    #[test]
    fn evaluates_unary_minus_error_on_non_number() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Minus, "-", 3),
            right: Box::new(Expr::Literal {
                value: Value::String("mano".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr);
        assert!(matches!(result, Err(ManoError::Runtime { line: 3, .. })));
    }

    #[test]
    fn evaluates_unary_bang_on_false() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Bang, "!", 1),
            right: Box::new(Expr::Literal {
                value: Value::Bool(false),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_unary_bang_on_true() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Bang, "!", 1),
            right: Box::new(Expr::Literal {
                value: Value::Bool(true),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn evaluates_unary_bang_on_nil() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Bang, "!", 1),
            right: Box::new(Expr::Literal { value: Value::Nil }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true)); // nil is falsey
    }

    #[test]
    fn evaluates_unary_bang_on_number() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Bang, "!", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(0.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(false)); // numbers are truthy
    }

    // === binary arithmetic ===

    #[test]
    fn evaluates_binary_plus() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(3.0),
            }),
            operator: make_token(crate::token::TokenType::Plus, "+", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(2.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(5.0));
    }

    #[test]
    fn evaluates_binary_minus() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
            operator: make_token(crate::token::TokenType::Minus, "-", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(3.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(2.0));
    }

    #[test]
    fn evaluates_binary_star() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(4.0),
            }),
            operator: make_token(crate::token::TokenType::Star, "*", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(3.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(12.0));
    }

    #[test]
    fn evaluates_binary_slash() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(10.0),
            }),
            operator: make_token(crate::token::TokenType::Slash, "/", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(2.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(5.0));
    }

    #[test]
    fn evaluates_binary_minus_error_on_non_numbers() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::String("mano".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Minus, "-", 2),
            right: Box::new(Expr::Literal {
                value: Value::Number(1.0),
            }),
        };
        let result = interpreter.interpret(&expr);
        assert!(matches!(result, Err(ManoError::Runtime { line: 2, .. })));
    }

    // === string concatenation ===

    #[test]
    fn evaluates_string_concatenation() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::String("salve ".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Plus, "+", 1),
            right: Box::new(Expr::Literal {
                value: Value::String("mano".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::String("salve mano".to_string()));
    }

    #[test]
    fn evaluates_plus_error_on_mixed_types() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::String("mano".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Plus, "+", 3),
            right: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
        };
        let result = interpreter.interpret(&expr);
        assert!(matches!(result, Err(ManoError::Runtime { line: 3, .. })));
    }

    // === binary comparison ===

    #[test]
    fn evaluates_greater() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
            operator: make_token(crate::token::TokenType::Greater, ">", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(3.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_greater_equal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
            operator: make_token(crate::token::TokenType::GreaterEqual, ">=", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_less() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(3.0),
            }),
            operator: make_token(crate::token::TokenType::Less, "<", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_less_equal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
            operator: make_token(crate::token::TokenType::LessEqual, "<=", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(5.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_comparison_error_on_non_numbers() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::String("a".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Greater, ">", 4),
            right: Box::new(Expr::Literal {
                value: Value::String("b".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr);
        assert!(matches!(result, Err(ManoError::Runtime { line: 4, .. })));
    }

    // === binary equality ===

    #[test]
    fn evaluates_equal_numbers() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
            operator: make_token(crate::token::TokenType::EqualEqual, "==", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_not_equal_numbers() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
            operator: make_token(crate::token::TokenType::BangEqual, "!=", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(99.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_nil_equals_nil() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal { value: Value::Nil }),
            operator: make_token(crate::token::TokenType::EqualEqual, "==", 1),
            right: Box::new(Expr::Literal { value: Value::Nil }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn evaluates_mixed_types_not_equal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(3.0),
            }),
            operator: make_token(crate::token::TokenType::EqualEqual, "==", 1),
            right: Box::new(Expr::Literal {
                value: Value::String("three".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    // === comma operator ===

    #[test]
    fn evaluates_comma_returns_right() {
        let mut interpreter = Interpreter::new();
        // 1, 2 -> 2
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Value::Number(1.0),
            }),
            operator: make_token(crate::token::TokenType::Comma, ",", 1),
            right: Box::new(Expr::Literal {
                value: Value::Number(2.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(2.0));
    }

    // === ternary operator ===

    #[test]
    fn evaluates_ternary_true_condition() {
        let mut interpreter = Interpreter::new();
        // true ? 1 : 2 -> 1
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
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(1.0));
    }

    #[test]
    fn evaluates_ternary_false_condition() {
        let mut interpreter = Interpreter::new();
        // false ? 1 : 2 -> 2
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Literal {
                value: Value::Bool(false),
            }),
            then_branch: Box::new(Expr::Literal {
                value: Value::Number(1.0),
            }),
            else_branch: Box::new(Expr::Literal {
                value: Value::Number(2.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(2.0));
    }

    #[test]
    fn evaluates_ternary_with_truthy_number() {
        let mut interpreter = Interpreter::new();
        // 42 ? "yes" : "no" -> "yes"
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Literal {
                value: Value::Number(42.0),
            }),
            then_branch: Box::new(Expr::Literal {
                value: Value::String("yes".to_string()),
            }),
            else_branch: Box::new(Expr::Literal {
                value: Value::String("no".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::String("yes".to_string()));
    }

    #[test]
    fn evaluates_ternary_with_nil_condition() {
        let mut interpreter = Interpreter::new();
        // nil ? "yes" : "no" -> "no"
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Literal { value: Value::Nil }),
            then_branch: Box::new(Expr::Literal {
                value: Value::String("yes".to_string()),
            }),
            else_branch: Box::new(Expr::Literal {
                value: Value::String("no".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::String("no".to_string()));
    }

    #[test]
    fn evaluates_ternary_with_expression_branches() {
        let mut interpreter = Interpreter::new();
        // (5 > 3) ? (10 + 5) : (10 - 5) -> 15
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Value::Number(5.0),
                }),
                operator: make_token(crate::token::TokenType::Greater, ">", 1),
                right: Box::new(Expr::Literal {
                    value: Value::Number(3.0),
                }),
            }),
            then_branch: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Value::Number(10.0),
                }),
                operator: make_token(crate::token::TokenType::Plus, "+", 1),
                right: Box::new(Expr::Literal {
                    value: Value::Number(5.0),
                }),
            }),
            else_branch: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Value::Number(10.0),
                }),
                operator: make_token(crate::token::TokenType::Minus, "-", 1),
                right: Box::new(Expr::Literal {
                    value: Value::Number(5.0),
                }),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Number(15.0));
    }
}
