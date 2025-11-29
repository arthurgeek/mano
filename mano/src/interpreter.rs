use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use crate::ast::{Expr, Stmt};
use crate::environment::Environment;
use crate::error::ManoError;
use crate::token::{TokenType, Value};

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            environment: Rc::new(RefCell::new(Environment::new())),
        }
    }

    pub fn variable_names(&self) -> Vec<String> {
        self.environment.borrow().variable_names()
    }

    pub fn execute(&mut self, stmt: &Stmt, output: &mut dyn Write) -> Result<(), ManoError> {
        match stmt {
            Stmt::Print { expression } => {
                let value = self.interpret(expression)?;
                writeln!(output, "{}", value)?;
                Ok(())
            }
            Stmt::Expression { expression } => {
                self.interpret(expression)?;
                Ok(())
            }
            Stmt::Var { name, initializer } => {
                match initializer {
                    Some(expr) => {
                        let value = self.interpret(expr)?;
                        self.environment
                            .borrow_mut()
                            .define(name.lexeme.clone(), value);
                    }
                    None => {
                        self.environment
                            .borrow_mut()
                            .define_uninitialized(name.lexeme.clone());
                    }
                };
                Ok(())
            }
            Stmt::Block { statements } => self.execute_block(statements, output),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_value = self.interpret(condition)?;
                if self.is_truthy(&condition_value) {
                    self.execute(then_branch, output)
                } else if let Some(else_stmt) = else_branch {
                    self.execute(else_stmt, output)
                } else {
                    Ok(())
                }
            }
            Stmt::While { condition, body } => {
                loop {
                    let condition_value = self.interpret(condition)?;
                    if !self.is_truthy(&condition_value) {
                        break;
                    }
                    match self.execute(body, output) {
                        Ok(()) => {}
                        Err(ManoError::Break) => break,
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            Stmt::Break => Err(ManoError::Break),
        }
    }

    fn execute_block(
        &mut self,
        statements: &[Stmt],
        output: &mut dyn Write,
    ) -> Result<(), ManoError> {
        let previous = Rc::clone(&self.environment);
        self.environment = Rc::new(RefCell::new(Environment::with_enclosing(Rc::clone(
            &previous,
        ))));

        let result = statements
            .iter()
            .try_for_each(|stmt| self.execute(stmt, output));

        self.environment = previous;
        result
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
                            message: "Só dá pra negar número, tio!".to_string(),
                            span: operator.span.clone(),
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
                        let (a, b) =
                            self.require_numbers(&left_val, &right_val, operator.span.clone())?;
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
                            message: "Só dá pra somar número com número ou texto com texto, chapa!"
                                .to_string(),
                            span: operator.span.clone(),
                        }),
                    },
                    TokenType::Greater
                    | TokenType::GreaterEqual
                    | TokenType::Less
                    | TokenType::LessEqual => {
                        let (a, b) =
                            self.require_numbers(&left_val, &right_val, operator.span.clone())?;
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
            Expr::Variable { name } => self
                .environment
                .borrow()
                .get(&name.lexeme, name.span.clone()),
            Expr::Assign { name, value } => {
                let val = self.interpret(value)?;
                self.environment.borrow_mut().assign(
                    &name.lexeme,
                    val.clone(),
                    name.span.clone(),
                )?;
                Ok(val)
            }
            Expr::Logical {
                left,
                operator,
                right,
            } => {
                let left_val = self.interpret(left)?;

                if operator.token_type == TokenType::Or {
                    if self.is_truthy(&left_val) {
                        return Ok(left_val);
                    }
                } else {
                    // And
                    if !self.is_truthy(&left_val) {
                        return Ok(left_val);
                    }
                }

                self.interpret(right)
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
        span: std::ops::Range<usize>,
    ) -> Result<(f64, f64), ManoError> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => Ok((*a, *b)),
            _ => Err(ManoError::Runtime {
                message: "Os dois lados precisam ser número, irmão!".to_string(),
                span,
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
        start: usize,
    ) -> crate::token::Token {
        crate::token::Token {
            token_type,
            lexeme: lexeme.to_string(),
            literal: None,
            span: start..start + lexeme.len(),
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
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
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
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
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
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
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
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
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

    // === statements ===

    #[test]
    fn executes_print_statement() {
        let mut interpreter = Interpreter::new();
        let stmt = Stmt::Print {
            expression: Expr::Literal {
                value: Value::Number(42.0),
            },
        };
        let mut output = Vec::new();
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn executes_expression_statement() {
        let mut interpreter = Interpreter::new();
        let stmt = Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Value::Number(1.0),
                }),
                operator: make_token(crate::token::TokenType::Plus, "+", 1),
                right: Box::new(Expr::Literal {
                    value: Value::Number(2.0),
                }),
            },
        };
        let mut output = Vec::new();
        // Expression statement evaluates but doesn't output
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(output.len(), 0);
    }

    #[test]
    fn print_statement_propagates_runtime_error() {
        let mut interpreter = Interpreter::new();
        // salve 1 + "mano"; -> runtime error (can't add number and string)
        let stmt = Stmt::Print {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Value::Number(1.0),
                }),
                operator: make_token(crate::token::TokenType::Plus, "+", 1),
                right: Box::new(Expr::Literal {
                    value: Value::String("mano".to_string()),
                }),
            },
        };
        let mut output = Vec::new();
        let result = interpreter.execute(&stmt, &mut output);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === variable declaration ===

    #[test]
    fn executes_var_declaration_and_access() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 42;
        let var_stmt = Stmt::Var {
            name: crate::token::Token {
                token_type: crate::token::TokenType::Identifier,
                lexeme: "x".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: Some(Expr::Literal {
                value: Value::Number(42.0),
            }),
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // salve x;
        let print_stmt = Stmt::Print {
            expression: Expr::Variable {
                name: crate::token::Token {
                    token_type: crate::token::TokenType::Identifier,
                    lexeme: "x".to_string(),
                    literal: None,
                    span: 0..1,
                },
            },
        };
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn executes_assignment() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::Var {
            name: crate::token::Token {
                token_type: crate::token::TokenType::Identifier,
                lexeme: "x".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: Some(Expr::Literal {
                value: Value::Number(1.0),
            }),
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // x = 42;
        let assign_stmt = Stmt::Expression {
            expression: Expr::Assign {
                name: crate::token::Token {
                    token_type: crate::token::TokenType::Identifier,
                    lexeme: "x".to_string(),
                    literal: None,
                    span: 0..1,
                },
                value: Box::new(Expr::Literal {
                    value: Value::Number(42.0),
                }),
            },
        };
        interpreter.execute(&assign_stmt, &mut output).unwrap();

        // salve x;
        let print_stmt = Stmt::Print {
            expression: Expr::Variable {
                name: crate::token::Token {
                    token_type: crate::token::TokenType::Identifier,
                    lexeme: "x".to_string(),
                    literal: None,
                    span: 0..1,
                },
            },
        };
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn accessing_uninitialized_variable_errors() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x;
        let var_stmt = Stmt::Var {
            name: crate::token::Token {
                token_type: crate::token::TokenType::Identifier,
                lexeme: "x".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: None,
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // salve x; -- should error!
        let print_stmt = Stmt::Print {
            expression: Expr::Variable {
                name: crate::token::Token {
                    token_type: crate::token::TokenType::Identifier,
                    lexeme: "x".to_string(),
                    literal: None,
                    span: 0..1,
                },
            },
        };
        let result = interpreter.execute(&print_stmt, &mut output);

        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn assigning_uninitialized_variable_works() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x;
        let var_stmt = Stmt::Var {
            name: crate::token::Token {
                token_type: crate::token::TokenType::Identifier,
                lexeme: "x".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: None,
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // x = 42;
        let assign_stmt = Stmt::Expression {
            expression: Expr::Assign {
                name: crate::token::Token {
                    token_type: crate::token::TokenType::Identifier,
                    lexeme: "x".to_string(),
                    literal: None,
                    span: 0..1,
                },
                value: Box::new(Expr::Literal {
                    value: Value::Number(42.0),
                }),
            },
        };
        interpreter.execute(&assign_stmt, &mut output).unwrap();

        // salve x;
        let print_stmt = Stmt::Print {
            expression: Expr::Variable {
                name: crate::token::Token {
                    token_type: crate::token::TokenType::Identifier,
                    lexeme: "x".to_string(),
                    literal: None,
                    span: 0..1,
                },
            },
        };
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    // === block statements ===

    #[test]
    fn executes_block_with_statements() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // { salve 1; salve 2; }
        let block = Stmt::Block {
            statements: vec![
                Stmt::Print {
                    expression: Expr::Literal {
                        value: Value::Number(1.0),
                    },
                },
                Stmt::Print {
                    expression: Expr::Literal {
                        value: Value::Number(2.0),
                    },
                },
            ],
        };
        interpreter.execute(&block, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n2\n");
    }

    #[test]
    fn block_scope_does_not_leak() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // { seLiga x = 42; }
        let block = Stmt::Block {
            statements: vec![Stmt::Var {
                name: make_token(crate::token::TokenType::Identifier, "x", 1),
                initializer: Some(Expr::Literal {
                    value: Value::Number(42.0),
                }),
            }],
        };
        interpreter.execute(&block, &mut output).unwrap();

        // x; (should error - x not defined in outer scope)
        let var_expr = Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 2),
        };
        let result = interpreter.interpret(&var_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn block_reads_outer_scope() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 42;
        let var_stmt = Stmt::Var {
            name: make_token(crate::token::TokenType::Identifier, "x", 1),
            initializer: Some(Expr::Literal {
                value: Value::Number(42.0),
            }),
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // { salve x; }
        let block = Stmt::Block {
            statements: vec![Stmt::Print {
                expression: Expr::Variable {
                    name: make_token(crate::token::TokenType::Identifier, "x", 2),
                },
            }],
        };
        interpreter.execute(&block, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn block_shadows_outer_scope() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::Var {
            name: make_token(crate::token::TokenType::Identifier, "x", 1),
            initializer: Some(Expr::Literal {
                value: Value::Number(1.0),
            }),
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // { seLiga x = 99; salve x; }
        let block = Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token(crate::token::TokenType::Identifier, "x", 2),
                    initializer: Some(Expr::Literal {
                        value: Value::Number(99.0),
                    }),
                },
                Stmt::Print {
                    expression: Expr::Variable {
                        name: make_token(crate::token::TokenType::Identifier, "x", 3),
                    },
                },
            ],
        };
        interpreter.execute(&block, &mut output).unwrap();

        // salve x; (should be 1 again)
        let print_stmt = Stmt::Print {
            expression: Expr::Variable {
                name: make_token(crate::token::TokenType::Identifier, "x", 4),
            },
        };
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "99\n1\n");
    }

    #[test]
    fn block_assignment_updates_outer_scope() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::Var {
            name: make_token(crate::token::TokenType::Identifier, "x", 1),
            initializer: Some(Expr::Literal {
                value: Value::Number(1.0),
            }),
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // { x = 99; }
        let block = Stmt::Block {
            statements: vec![Stmt::Expression {
                expression: Expr::Assign {
                    name: make_token(crate::token::TokenType::Identifier, "x", 2),
                    value: Box::new(Expr::Literal {
                        value: Value::Number(99.0),
                    }),
                },
            }],
        };
        interpreter.execute(&block, &mut output).unwrap();

        // salve x; (should be 99)
        let print_stmt = Stmt::Print {
            expression: Expr::Variable {
                name: make_token(crate::token::TokenType::Identifier, "x", 3),
            },
        };
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "99\n");
    }

    #[test]
    fn block_error_restores_environment() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::Var {
            name: make_token(crate::token::TokenType::Identifier, "x", 1),
            initializer: Some(Expr::Literal {
                value: Value::Number(1.0),
            }),
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // { seLiga y = 99; undefined_var; } - should error on undefined_var
        let block = Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token(crate::token::TokenType::Identifier, "y", 2),
                    initializer: Some(Expr::Literal {
                        value: Value::Number(99.0),
                    }),
                },
                Stmt::Expression {
                    expression: Expr::Variable {
                        name: make_token(crate::token::TokenType::Identifier, "undefined_var", 3),
                    },
                },
            ],
        };
        let result = interpreter.execute(&block, &mut output);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));

        // x should still be accessible (environment restored)
        let var_expr = Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 4),
        };
        let result = interpreter.interpret(&var_expr).unwrap();
        assert_eq!(result, Value::Number(1.0));

        // y should NOT be accessible (was in block scope)
        let var_expr = Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "y", 5),
        };
        let result = interpreter.interpret(&var_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === if statements ===

    #[test]
    fn executes_if_true_branch() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (firmeza) salve 1;
        let stmt = Stmt::If {
            condition: Expr::Literal {
                value: Value::Bool(true),
            },
            then_branch: Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(1.0),
                },
            }),
            else_branch: None,
        };
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn executes_if_false_skips_then() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (treta) salve 1;
        let stmt = Stmt::If {
            condition: Expr::Literal {
                value: Value::Bool(false),
            },
            then_branch: Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(1.0),
                },
            }),
            else_branch: None,
        };
        interpreter.execute(&stmt, &mut output).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn executes_if_else_true_branch() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (firmeza) salve 1; vacilou salve 2;
        let stmt = Stmt::If {
            condition: Expr::Literal {
                value: Value::Bool(true),
            },
            then_branch: Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(1.0),
                },
            }),
            else_branch: Some(Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(2.0),
                },
            })),
        };
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn executes_if_else_false_branch() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (treta) salve 1; vacilou salve 2;
        let stmt = Stmt::If {
            condition: Expr::Literal {
                value: Value::Bool(false),
            },
            then_branch: Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(1.0),
                },
            }),
            else_branch: Some(Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(2.0),
                },
            })),
        };
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "2\n");
    }

    #[test]
    fn if_uses_truthiness() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (nadaNão) salve 1; vacilou salve 2;
        let stmt = Stmt::If {
            condition: Expr::Literal { value: Value::Nil },
            then_branch: Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(1.0),
                },
            }),
            else_branch: Some(Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(2.0),
                },
            })),
        };
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "2\n");
    }

    // === logical operators ===

    #[test]
    fn or_returns_left_if_truthy() {
        let mut interpreter = Interpreter::new();
        // "hi" ow 2 -> "hi"
        let expr = Expr::Logical {
            left: Box::new(Expr::Literal {
                value: Value::String("hi".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Or, "ow", 0),
            right: Box::new(Expr::Literal {
                value: Value::Number(2.0),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::String("hi".to_string()));
    }

    #[test]
    fn or_returns_right_if_left_falsy() {
        let mut interpreter = Interpreter::new();
        // nadaNão ow "fallback" -> "fallback"
        let expr = Expr::Logical {
            left: Box::new(Expr::Literal { value: Value::Nil }),
            operator: make_token(crate::token::TokenType::Or, "ow", 0),
            right: Box::new(Expr::Literal {
                value: Value::String("fallback".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::String("fallback".to_string()));
    }

    #[test]
    fn and_returns_left_if_falsy() {
        let mut interpreter = Interpreter::new();
        // treta tamoJunto "never" -> treta
        let expr = Expr::Logical {
            left: Box::new(Expr::Literal {
                value: Value::Bool(false),
            }),
            operator: make_token(crate::token::TokenType::And, "tamoJunto", 0),
            right: Box::new(Expr::Literal {
                value: Value::String("never".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn and_returns_right_if_left_truthy() {
        let mut interpreter = Interpreter::new();
        // firmeza tamoJunto "yes" -> "yes"
        let expr = Expr::Logical {
            left: Box::new(Expr::Literal {
                value: Value::Bool(true),
            }),
            operator: make_token(crate::token::TokenType::And, "tamoJunto", 0),
            right: Box::new(Expr::Literal {
                value: Value::String("yes".to_string()),
            }),
        };
        let result = interpreter.interpret(&expr).unwrap();
        assert_eq!(result, Value::String("yes".to_string()));
    }

    // === while statements ===

    #[test]
    fn executes_while_loop() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 0;
        let var_stmt = Stmt::Var {
            name: make_token(crate::token::TokenType::Identifier, "x", 0),
            initializer: Some(Expr::Literal {
                value: Value::Number(0.0),
            }),
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // segueOFluxo (x < 3) { salve x; x = x + 1; }
        let while_stmt = Stmt::While {
            condition: Expr::Binary {
                left: Box::new(Expr::Variable {
                    name: make_token(crate::token::TokenType::Identifier, "x", 0),
                }),
                operator: make_token(crate::token::TokenType::Less, "<", 0),
                right: Box::new(Expr::Literal {
                    value: Value::Number(3.0),
                }),
            },
            body: Box::new(Stmt::Block {
                statements: vec![
                    Stmt::Print {
                        expression: Expr::Variable {
                            name: make_token(crate::token::TokenType::Identifier, "x", 0),
                        },
                    },
                    Stmt::Expression {
                        expression: Expr::Assign {
                            name: make_token(crate::token::TokenType::Identifier, "x", 0),
                            value: Box::new(Expr::Binary {
                                left: Box::new(Expr::Variable {
                                    name: make_token(crate::token::TokenType::Identifier, "x", 0),
                                }),
                                operator: make_token(crate::token::TokenType::Plus, "+", 0),
                                right: Box::new(Expr::Literal {
                                    value: Value::Number(1.0),
                                }),
                            }),
                        },
                    },
                ],
            }),
        };
        interpreter.execute(&while_stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "0\n1\n2\n");
    }

    #[test]
    fn while_false_never_executes() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // segueOFluxo (treta) salve 1;
        let stmt = Stmt::While {
            condition: Expr::Literal {
                value: Value::Bool(false),
            },
            body: Box::new(Stmt::Print {
                expression: Expr::Literal {
                    value: Value::Number(1.0),
                },
            }),
        };
        interpreter.execute(&stmt, &mut output).unwrap();
        assert!(output.is_empty());
    }

    // === break statements ===

    #[test]
    fn break_exits_while_loop() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga i = 0;
        let var_stmt = Stmt::Var {
            name: make_token(crate::token::TokenType::Identifier, "i", 0),
            initializer: Some(Expr::Literal {
                value: Value::Number(0.0),
            }),
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // segueOFluxo (firmeza) { salve i; sePá (i == 2) saiFora; i = i + 1; }
        let while_stmt = Stmt::While {
            condition: Expr::Literal {
                value: Value::Bool(true),
            },
            body: Box::new(Stmt::Block {
                statements: vec![
                    Stmt::Print {
                        expression: Expr::Variable {
                            name: make_token(crate::token::TokenType::Identifier, "i", 0),
                        },
                    },
                    Stmt::If {
                        condition: Expr::Binary {
                            left: Box::new(Expr::Variable {
                                name: make_token(crate::token::TokenType::Identifier, "i", 0),
                            }),
                            operator: make_token(crate::token::TokenType::EqualEqual, "==", 0),
                            right: Box::new(Expr::Literal {
                                value: Value::Number(2.0),
                            }),
                        },
                        then_branch: Box::new(Stmt::Break),
                        else_branch: None,
                    },
                    Stmt::Expression {
                        expression: Expr::Assign {
                            name: make_token(crate::token::TokenType::Identifier, "i", 0),
                            value: Box::new(Expr::Binary {
                                left: Box::new(Expr::Variable {
                                    name: make_token(crate::token::TokenType::Identifier, "i", 0),
                                }),
                                operator: make_token(crate::token::TokenType::Plus, "+", 0),
                                right: Box::new(Expr::Literal {
                                    value: Value::Number(1.0),
                                }),
                            }),
                        },
                    },
                ],
            }),
        };
        interpreter.execute(&while_stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "0\n1\n2\n");
    }

    #[test]
    fn break_exits_immediately() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // segueOFluxo (firmeza) { salve 1; saiFora; salve 2; }
        let stmt = Stmt::While {
            condition: Expr::Literal {
                value: Value::Bool(true),
            },
            body: Box::new(Stmt::Block {
                statements: vec![
                    Stmt::Print {
                        expression: Expr::Literal {
                            value: Value::Number(1.0),
                        },
                    },
                    Stmt::Break,
                    Stmt::Print {
                        expression: Expr::Literal {
                            value: Value::Number(2.0),
                        },
                    },
                ],
            }),
        };
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn while_propagates_runtime_error() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // segueOFluxo (firmeza) { salve -"oops"; }
        let stmt = Stmt::While {
            condition: Expr::Literal {
                value: Value::Bool(true),
            },
            body: Box::new(Stmt::Print {
                expression: Expr::Unary {
                    operator: make_token(crate::token::TokenType::Minus, "-", 0),
                    right: Box::new(Expr::Literal {
                        value: Value::String("oops".to_string()),
                    }),
                },
            }),
        };
        let result = interpreter.execute(&stmt, &mut output);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }
}
