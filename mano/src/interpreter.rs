use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::time::SystemTime;

use crate::ast::{Expr, Stmt};
use crate::environment::Environment;
use crate::error::ManoError;
use crate::token::{Literal, TokenType};
use crate::value::{Function, ManoFunction, NativeFunction, Value};

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
}

impl Interpreter {
    pub fn new() -> Self {
        let environment = Rc::new(RefCell::new(Environment::new()));

        // Register native function: fazTeuCorre (clock)
        let faz_teu_corre = NativeFunction {
            name: "fazTeuCorre".to_string(),
            arity: 0,
            func: |_| {
                let time = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64();
                Ok(Value::Literal(Literal::Number(time)))
            },
        };
        environment.borrow_mut().define(
            "fazTeuCorre".to_string(),
            Value::Function(Rc::new(Function::Native(faz_teu_corre))),
        );

        Self { environment }
    }

    pub fn variable_names(&self) -> Vec<String> {
        self.environment.borrow().variable_names()
    }

    pub fn execute(&mut self, stmt: &Stmt, output: &mut dyn Write) -> Result<(), ManoError> {
        match stmt {
            Stmt::Print { expression, .. } => {
                let value = self.interpret(expression, output)?;
                writeln!(output, "{}", value)?;
                Ok(())
            }
            Stmt::Expression { expression, .. } => {
                self.interpret(expression, output)?;
                Ok(())
            }
            Stmt::Var {
                name, initializer, ..
            } => {
                if let Some(expr) = initializer {
                    let value = self.interpret(expr, output)?;
                    self.environment
                        .borrow_mut()
                        .define(name.lexeme.clone(), value);
                } else {
                    self.environment
                        .borrow_mut()
                        .define_uninitialized(name.lexeme.clone());
                }
                Ok(())
            }
            Stmt::Block { statements, .. } => self.execute_block(statements, output),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_value = self.interpret(condition, output)?;
                if self.is_truthy(&condition_value) {
                    self.execute(then_branch, output)
                } else if let Some(else_stmt) = else_branch {
                    self.execute(else_stmt, output)
                } else {
                    Ok(())
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                loop {
                    let condition_value = self.interpret(condition, output)?;
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
            Stmt::Break { .. } => Err(ManoError::Break),
            Stmt::Else { body, .. } => self.execute(body, output),
            Stmt::Function {
                name, params, body, ..
            } => {
                let function = ManoFunction {
                    name: Some(name.clone()),
                    params: params.clone(),
                    body: body.clone(),
                    closure: Rc::clone(&self.environment),
                };
                self.environment.borrow_mut().define(
                    name.lexeme.clone(),
                    Value::Function(Rc::new(Function::Mano(function))),
                );
                Ok(())
            }
            Stmt::Return { value, .. } => {
                let return_value = match value {
                    Some(expr) => self.interpret(expr, output)?,
                    None => Value::Literal(Literal::Nil),
                };
                Err(ManoError::Return(return_value))
            }
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

    pub fn interpret(&mut self, expr: &Expr, output: &mut dyn Write) -> Result<Value, ManoError> {
        match expr {
            Expr::Literal { value } => Ok(Value::Literal(value.clone())),
            Expr::Grouping { expression } => self.interpret(expression, output),
            Expr::Unary { operator, right } => {
                let right_val = self.interpret(right, output)?;
                match operator.token_type {
                    TokenType::Minus => match right_val {
                        Value::Literal(Literal::Number(n)) => {
                            Ok(Value::Literal(Literal::Number(-n)))
                        }
                        _ => Err(ManoError::Runtime {
                            message: "Só dá pra negar número, tio!".to_string(),
                            span: operator.span.clone(),
                        }),
                    },
                    TokenType::Bang => {
                        Ok(Value::Literal(Literal::Bool(!self.is_truthy(&right_val))))
                    }
                    _ => unreachable!(),
                }
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let left_val = self.interpret(left, output)?;
                let right_val = self.interpret(right, output)?;

                match operator.token_type {
                    TokenType::Minus | TokenType::Slash | TokenType::Star | TokenType::Percent => {
                        let (a, b) =
                            self.require_numbers(&left_val, &right_val, operator.span.clone())?;
                        match operator.token_type {
                            TokenType::Minus => Ok(Value::Literal(Literal::Number(a - b))),
                            TokenType::Slash => Ok(Value::Literal(Literal::Number(a / b))),
                            TokenType::Star => Ok(Value::Literal(Literal::Number(a * b))),
                            TokenType::Percent => Ok(Value::Literal(Literal::Number(a % b))),
                            _ => unreachable!(),
                        }
                    }
                    TokenType::Plus => match (&left_val, &right_val) {
                        (
                            Value::Literal(Literal::Number(a)),
                            Value::Literal(Literal::Number(b)),
                        ) => Ok(Value::Literal(Literal::Number(a + b))),
                        (
                            Value::Literal(Literal::String(a)),
                            Value::Literal(Literal::String(b)),
                        ) => Ok(Value::Literal(Literal::String(format!("{}{}", a, b)))),
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
                        Ok(Value::Literal(Literal::Bool(result)))
                    }
                    TokenType::EqualEqual => Ok(Value::Literal(Literal::Bool(
                        self.is_equal(&left_val, &right_val),
                    ))),
                    TokenType::BangEqual => Ok(Value::Literal(Literal::Bool(
                        !self.is_equal(&left_val, &right_val),
                    ))),
                    TokenType::Comma => Ok(right_val),
                    _ => unreachable!(),
                }
            }
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.interpret(condition, output)?;
                if self.is_truthy(&cond_val) {
                    self.interpret(then_branch, output)
                } else {
                    self.interpret(else_branch, output)
                }
            }
            Expr::Variable { name } => self
                .environment
                .borrow()
                .get(&name.lexeme, name.span.clone()),
            Expr::Assign { name, value } => {
                let val = self.interpret(value, output)?;
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
                let left_val = self.interpret(left, output)?;

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

                self.interpret(right, output)
            }
            Expr::Call {
                callee,
                paren,
                arguments,
            } => {
                let callee_val = self.interpret(callee, output)?;

                let mut args = Vec::new();
                for arg in arguments {
                    args.push(self.interpret(arg, output)?);
                }

                match callee_val {
                    Value::Function(func) => match func.as_ref() {
                        Function::Mano(mano_func) => {
                            if args.len() != mano_func.params.len() {
                                return Err(ManoError::Runtime {
                                    message: format!(
                                        "Essa fita espera {} argumentos, mas tu passou {}, maluco!",
                                        mano_func.params.len(),
                                        args.len()
                                    ),
                                    span: paren.span.clone(),
                                });
                            }
                            // Clone to avoid borrowing issues
                            let func_clone = ManoFunction {
                                name: mano_func.name.clone(),
                                params: mano_func.params.clone(),
                                body: mano_func.body.clone(),
                                closure: Rc::clone(&mano_func.closure),
                            };
                            self.call_mano_function(&func_clone, args, output)
                        }
                        Function::Native(native_func) => {
                            if args.len() != native_func.arity {
                                return Err(ManoError::Runtime {
                                    message: format!(
                                        "Essa fita raiz espera {} argumentos, mas tu passou {}, véi!",
                                        native_func.arity,
                                        args.len()
                                    ),
                                    span: paren.span.clone(),
                                });
                            }
                            (native_func.func)(&args)
                        }
                    },
                    _ => Err(ManoError::Runtime {
                        message: "Só dá pra chamar fita, chapa!".to_string(),
                        span: paren.span.clone(),
                    }),
                }
            }
            Expr::Lambda { params, body } => {
                let func = ManoFunction {
                    name: None,
                    params: params.clone(),
                    body: body.clone(),
                    closure: Rc::clone(&self.environment),
                };
                Ok(Value::Function(Rc::new(Function::Mano(func))))
            }
        }
    }

    fn call_mano_function(
        &mut self,
        func: &ManoFunction,
        args: Vec<Value>,
        output: &mut dyn Write,
    ) -> Result<Value, ManoError> {
        let previous = Rc::clone(&self.environment);

        // Create new environment with closure as enclosing
        self.environment = Rc::new(RefCell::new(Environment::with_enclosing(Rc::clone(
            &func.closure,
        ))));

        // Bind parameters to arguments
        for (param, arg) in func.params.iter().zip(args.into_iter()) {
            self.environment
                .borrow_mut()
                .define(param.lexeme.clone(), arg);
        }

        // Execute body
        let mut return_value = Value::Literal(Literal::Nil);
        for stmt in &func.body {
            match self.execute(stmt, output) {
                Ok(()) => {}
                Err(ManoError::Return(value)) => {
                    return_value = value;
                    break;
                }
                Err(e) => {
                    self.environment = previous;
                    return Err(e);
                }
            }
        }

        // Restore environment
        self.environment = previous;

        Ok(return_value)
    }

    fn is_truthy(&self, value: &Value) -> bool {
        match value {
            Value::Literal(Literal::Nil) => false,
            Value::Literal(Literal::Bool(b)) => *b,
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
            (Value::Literal(Literal::Number(a)), Value::Literal(Literal::Number(b))) => {
                Ok((*a, *b))
            }
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

    fn num(n: f64) -> Value {
        Value::Literal(Literal::Number(n))
    }

    fn str(s: &str) -> Value {
        Value::Literal(Literal::String(s.to_string()))
    }

    fn bool_val(b: bool) -> Value {
        Value::Literal(Literal::Bool(b))
    }

    fn nil() -> Value {
        Value::Literal(Literal::Nil)
    }

    fn eval(interpreter: &mut Interpreter, expr: &Expr) -> Result<Value, ManoError> {
        interpreter.interpret(expr, &mut std::io::sink())
    }

    // === literals ===

    #[test]
    fn evaluates_number_literal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Literal::Number(42.0),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn evaluates_string_literal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Literal::String("mano".to_string()),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, str("mano"));
    }

    #[test]
    fn evaluates_bool_true() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Literal::Bool(true),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_bool_false() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Literal::Bool(false),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(false));
    }

    #[test]
    fn evaluates_nil() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Literal {
            value: Literal::Nil,
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, nil());
    }

    // === grouping ===

    #[test]
    fn evaluates_grouping() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Grouping {
            expression: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(42.0));
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
                value: Literal::Number(5.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(-5.0));
    }

    #[test]
    fn evaluates_unary_minus_error_on_non_number() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Minus, "-", 3),
            right: Box::new(Expr::Literal {
                value: Literal::String("mano".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn evaluates_unary_bang_on_false() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Bang, "!", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Bool(false),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_unary_bang_on_true() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Bang, "!", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Bool(true),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(false));
    }

    #[test]
    fn evaluates_unary_bang_on_nil() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Bang, "!", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Nil,
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true)); // nil is falsey
    }

    #[test]
    fn evaluates_unary_bang_on_number() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            operator: make_token(crate::token::TokenType::Bang, "!", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(0.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(false)); // numbers are truthy
    }

    // === binary arithmetic ===

    #[test]
    fn evaluates_binary_plus() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(3.0),
            }),
            operator: make_token(crate::token::TokenType::Plus, "+", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(5.0));
    }

    #[test]
    fn evaluates_binary_minus() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(5.0),
            }),
            operator: make_token(crate::token::TokenType::Minus, "-", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(3.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(2.0));
    }

    #[test]
    fn evaluates_binary_star() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(4.0),
            }),
            operator: make_token(crate::token::TokenType::Star, "*", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(3.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(12.0));
    }

    #[test]
    fn evaluates_binary_slash() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(10.0),
            }),
            operator: make_token(crate::token::TokenType::Slash, "/", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(5.0));
    }

    #[test]
    fn evaluates_binary_percent() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(10.0),
            }),
            operator: make_token(crate::token::TokenType::Percent, "%", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(3.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(1.0));
    }

    #[test]
    fn evaluates_binary_minus_error_on_non_numbers() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::String("mano".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Minus, "-", 2),
            right: Box::new(Expr::Literal {
                value: Literal::Number(1.0),
            }),
        };
        let result = eval(&mut interpreter, &expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === string concatenation ===

    #[test]
    fn evaluates_string_concatenation() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::String("salve ".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Plus, "+", 1),
            right: Box::new(Expr::Literal {
                value: Literal::String("mano".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, str("salve mano"));
    }

    #[test]
    fn evaluates_plus_error_on_mixed_types() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::String("mano".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Plus, "+", 3),
            right: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        };
        let result = eval(&mut interpreter, &expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === binary comparison ===

    #[test]
    fn evaluates_greater() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(5.0),
            }),
            operator: make_token(crate::token::TokenType::Greater, ">", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(3.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_greater_equal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(5.0),
            }),
            operator: make_token(crate::token::TokenType::GreaterEqual, ">=", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(5.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_less() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(3.0),
            }),
            operator: make_token(crate::token::TokenType::Less, "<", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(5.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_less_equal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(5.0),
            }),
            operator: make_token(crate::token::TokenType::LessEqual, "<=", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(5.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_comparison_error_on_non_numbers() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::String("a".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Greater, ">", 4),
            right: Box::new(Expr::Literal {
                value: Literal::String("b".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === binary equality ===

    #[test]
    fn evaluates_equal_numbers() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
            operator: make_token(crate::token::TokenType::EqualEqual, "==", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_not_equal_numbers() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
            operator: make_token(crate::token::TokenType::BangEqual, "!=", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(99.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_nil_equals_nil() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Nil,
            }),
            operator: make_token(crate::token::TokenType::EqualEqual, "==", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Nil,
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(true));
    }

    #[test]
    fn evaluates_mixed_types_not_equal() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(3.0),
            }),
            operator: make_token(crate::token::TokenType::EqualEqual, "==", 1),
            right: Box::new(Expr::Literal {
                value: Literal::String("three".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(false));
    }

    // === comma operator ===

    #[test]
    fn evaluates_comma_returns_right() {
        let mut interpreter = Interpreter::new();
        // 1, 2 -> 2
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            operator: make_token(crate::token::TokenType::Comma, ",", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(2.0));
    }

    // === ternary operator ===

    #[test]
    fn evaluates_ternary_true_condition() {
        let mut interpreter = Interpreter::new();
        // true ? 1 : 2 -> 1
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
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(1.0));
    }

    #[test]
    fn evaluates_ternary_false_condition() {
        let mut interpreter = Interpreter::new();
        // false ? 1 : 2 -> 2
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Literal {
                value: Literal::Bool(false),
            }),
            then_branch: Box::new(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            else_branch: Box::new(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(2.0));
    }

    #[test]
    fn evaluates_ternary_with_truthy_number() {
        let mut interpreter = Interpreter::new();
        // 42 ? "yes" : "no" -> "yes"
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
            then_branch: Box::new(Expr::Literal {
                value: Literal::String("yes".to_string()),
            }),
            else_branch: Box::new(Expr::Literal {
                value: Literal::String("no".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, str("yes"));
    }

    #[test]
    fn evaluates_ternary_with_nil_condition() {
        let mut interpreter = Interpreter::new();
        // nil ? "yes" : "no" -> "no"
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Literal {
                value: Literal::Nil,
            }),
            then_branch: Box::new(Expr::Literal {
                value: Literal::String("yes".to_string()),
            }),
            else_branch: Box::new(Expr::Literal {
                value: Literal::String("no".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, str("no"));
    }

    #[test]
    fn evaluates_ternary_with_expression_branches() {
        let mut interpreter = Interpreter::new();
        // (5 > 3) ? (10 + 5) : (10 - 5) -> 15
        let expr = Expr::Ternary {
            condition: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(5.0),
                }),
                operator: make_token(crate::token::TokenType::Greater, ">", 1),
                right: Box::new(Expr::Literal {
                    value: Literal::Number(3.0),
                }),
            }),
            then_branch: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(10.0),
                }),
                operator: make_token(crate::token::TokenType::Plus, "+", 1),
                right: Box::new(Expr::Literal {
                    value: Literal::Number(5.0),
                }),
            }),
            else_branch: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(10.0),
                }),
                operator: make_token(crate::token::TokenType::Minus, "-", 1),
                right: Box::new(Expr::Literal {
                    value: Literal::Number(5.0),
                }),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, num(15.0));
    }

    // === statements ===

    #[test]
    fn executes_print_statement() {
        let mut interpreter = Interpreter::new();
        let stmt = Stmt::print(Expr::Literal {
            value: Literal::Number(42.0),
        });
        let mut output = Vec::new();
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn executes_expression_statement() {
        let mut interpreter = Interpreter::new();
        let stmt = Stmt::expression(Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            operator: make_token(crate::token::TokenType::Plus, "+", 1),
            right: Box::new(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        });
        let mut output = Vec::new();
        // Expression statement evaluates but doesn't output
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(output.len(), 0);
    }

    #[test]
    fn print_statement_propagates_runtime_error() {
        let mut interpreter = Interpreter::new();
        // salve 1 + "mano"; -> runtime error (can't add number and string)
        let stmt = Stmt::print(Expr::Binary {
            left: Box::new(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            operator: make_token(crate::token::TokenType::Plus, "+", 1),
            right: Box::new(Expr::Literal {
                value: Literal::String("mano".to_string()),
            }),
        });
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
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 0),
            Some(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // salve x;
        let print_stmt = Stmt::print(Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 0),
        });
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn executes_assignment() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 0),
            Some(Expr::Literal {
                value: Literal::Number(1.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // x = 42;
        let assign_stmt = Stmt::expression(Expr::Assign {
            name: make_token(crate::token::TokenType::Identifier, "x", 0),
            value: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        });
        interpreter.execute(&assign_stmt, &mut output).unwrap();

        // salve x;
        let print_stmt = Stmt::print(Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 0),
        });
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn accessing_uninitialized_variable_errors() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 0),
            None,
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // salve x; -- should error!
        let print_stmt = Stmt::print(Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 0),
        });
        let result = interpreter.execute(&print_stmt, &mut output);

        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn assigning_uninitialized_variable_works() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 0),
            None,
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // x = 42;
        let assign_stmt = Stmt::expression(Expr::Assign {
            name: make_token(crate::token::TokenType::Identifier, "x", 0),
            value: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        });
        interpreter.execute(&assign_stmt, &mut output).unwrap();

        // salve x;
        let print_stmt = Stmt::print(Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 0),
        });
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    // === block statements ===

    #[test]
    fn executes_block_with_statements() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // { salve 1; salve 2; }
        let block = Stmt::block(vec![
            Stmt::print(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            Stmt::print(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        ]);
        interpreter.execute(&block, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n2\n");
    }

    #[test]
    fn block_scope_does_not_leak() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // { seLiga x = 42; }
        let block = Stmt::block(vec![Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 1),
            Some(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        )]);
        interpreter.execute(&block, &mut output).unwrap();

        // x; (should error - x not defined in outer scope)
        let var_expr = Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 2),
        };
        let result = eval(&mut interpreter, &var_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn block_reads_outer_scope() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 42;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 1),
            Some(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // { salve x; }
        let block = Stmt::block(vec![Stmt::print(Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 2),
        })]);
        interpreter.execute(&block, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn block_shadows_outer_scope() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 1),
            Some(Expr::Literal {
                value: Literal::Number(1.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // { seLiga x = 99; salve x; }
        let block = Stmt::block(vec![
            Stmt::var(
                make_token(crate::token::TokenType::Identifier, "x", 2),
                Some(Expr::Literal {
                    value: Literal::Number(99.0),
                }),
            ),
            Stmt::print(Expr::Variable {
                name: make_token(crate::token::TokenType::Identifier, "x", 3),
            }),
        ]);
        interpreter.execute(&block, &mut output).unwrap();

        // salve x; (should be 1 again)
        let print_stmt = Stmt::print(Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 4),
        });
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "99\n1\n");
    }

    #[test]
    fn block_assignment_updates_outer_scope() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 1),
            Some(Expr::Literal {
                value: Literal::Number(1.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // { x = 99; }
        let block = Stmt::block(vec![Stmt::expression(Expr::Assign {
            name: make_token(crate::token::TokenType::Identifier, "x", 2),
            value: Box::new(Expr::Literal {
                value: Literal::Number(99.0),
            }),
        })]);
        interpreter.execute(&block, &mut output).unwrap();

        // salve x; (should be 99)
        let print_stmt = Stmt::print(Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 3),
        });
        interpreter.execute(&print_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "99\n");
    }

    #[test]
    fn block_error_restores_environment() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 1),
            Some(Expr::Literal {
                value: Literal::Number(1.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // { seLiga y = 99; undefined_var; } - should error on undefined_var
        let block = Stmt::block(vec![
            Stmt::var(
                make_token(crate::token::TokenType::Identifier, "y", 2),
                Some(Expr::Literal {
                    value: Literal::Number(99.0),
                }),
            ),
            Stmt::expression(Expr::Variable {
                name: make_token(crate::token::TokenType::Identifier, "undefined_var", 3),
            }),
        ]);
        let result = interpreter.execute(&block, &mut output);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));

        // x should still be accessible (environment restored)
        let var_expr = Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 4),
        };
        let result = eval(&mut interpreter, &var_expr).unwrap();
        assert_eq!(result, num(1.0));

        // y should NOT be accessible (was in block scope)
        let var_expr = Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "y", 5),
        };
        let result = eval(&mut interpreter, &var_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === if statements ===

    #[test]
    fn executes_if_true_branch() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (firmeza) salve 1;
        let stmt = Stmt::if_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            Stmt::print(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            None,
        );
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn executes_if_false_skips_then() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (treta) salve 1;
        let stmt = Stmt::if_stmt(
            Expr::Literal {
                value: Literal::Bool(false),
            },
            Stmt::print(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            None,
        );
        interpreter.execute(&stmt, &mut output).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn executes_if_else_true_branch() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (firmeza) salve 1; vacilou salve 2;
        let stmt = Stmt::if_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            Stmt::print(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            Some(Stmt::print(Expr::Literal {
                value: Literal::Number(2.0),
            })),
        );
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn executes_if_else_false_branch() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (treta) salve 1; vacilou salve 2;
        let stmt = Stmt::if_stmt(
            Expr::Literal {
                value: Literal::Bool(false),
            },
            Stmt::print(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            Some(Stmt::print(Expr::Literal {
                value: Literal::Number(2.0),
            })),
        );
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "2\n");
    }

    #[test]
    fn if_uses_truthiness() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // sePá (nadaNão) salve 1; vacilou salve 2;
        let stmt = Stmt::if_stmt(
            Expr::Literal {
                value: Literal::Nil,
            },
            Stmt::print(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            Some(Stmt::print(Expr::Literal {
                value: Literal::Number(2.0),
            })),
        );
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
                value: Literal::String("hi".to_string()),
            }),
            operator: make_token(crate::token::TokenType::Or, "ow", 0),
            right: Box::new(Expr::Literal {
                value: Literal::Number(2.0),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, str("hi"));
    }

    #[test]
    fn or_returns_right_if_left_falsy() {
        let mut interpreter = Interpreter::new();
        // nadaNão ow "fallback" -> "fallback"
        let expr = Expr::Logical {
            left: Box::new(Expr::Literal {
                value: Literal::Nil,
            }),
            operator: make_token(crate::token::TokenType::Or, "ow", 0),
            right: Box::new(Expr::Literal {
                value: Literal::String("fallback".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, str("fallback"));
    }

    #[test]
    fn and_returns_left_if_falsy() {
        let mut interpreter = Interpreter::new();
        // treta tamoJunto "never" -> treta
        let expr = Expr::Logical {
            left: Box::new(Expr::Literal {
                value: Literal::Bool(false),
            }),
            operator: make_token(crate::token::TokenType::And, "tamoJunto", 0),
            right: Box::new(Expr::Literal {
                value: Literal::String("never".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, bool_val(false));
    }

    #[test]
    fn and_returns_right_if_left_truthy() {
        let mut interpreter = Interpreter::new();
        // firmeza tamoJunto "yes" -> "yes"
        let expr = Expr::Logical {
            left: Box::new(Expr::Literal {
                value: Literal::Bool(true),
            }),
            operator: make_token(crate::token::TokenType::And, "tamoJunto", 0),
            right: Box::new(Expr::Literal {
                value: Literal::String("yes".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr).unwrap();
        assert_eq!(result, str("yes"));
    }

    // === while statements ===

    #[test]
    fn executes_while_loop() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 0;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 0),
            Some(Expr::Literal {
                value: Literal::Number(0.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // segueOFluxo (x < 3) { salve x; x = x + 1; }
        let while_stmt = Stmt::while_stmt(
            Expr::Binary {
                left: Box::new(Expr::Variable {
                    name: make_token(crate::token::TokenType::Identifier, "x", 0),
                }),
                operator: make_token(crate::token::TokenType::Less, "<", 0),
                right: Box::new(Expr::Literal {
                    value: Literal::Number(3.0),
                }),
            },
            Stmt::block(vec![
                Stmt::print(Expr::Variable {
                    name: make_token(crate::token::TokenType::Identifier, "x", 0),
                }),
                Stmt::expression(Expr::Assign {
                    name: make_token(crate::token::TokenType::Identifier, "x", 0),
                    value: Box::new(Expr::Binary {
                        left: Box::new(Expr::Variable {
                            name: make_token(crate::token::TokenType::Identifier, "x", 0),
                        }),
                        operator: make_token(crate::token::TokenType::Plus, "+", 0),
                        right: Box::new(Expr::Literal {
                            value: Literal::Number(1.0),
                        }),
                    }),
                }),
            ]),
        );
        interpreter.execute(&while_stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "0\n1\n2\n");
    }

    #[test]
    fn while_false_never_executes() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // segueOFluxo (treta) salve 1;
        let stmt = Stmt::while_stmt(
            Expr::Literal {
                value: Literal::Bool(false),
            },
            Stmt::print(Expr::Literal {
                value: Literal::Number(1.0),
            }),
        );
        interpreter.execute(&stmt, &mut output).unwrap();
        assert!(output.is_empty());
    }

    // === break statements ===

    #[test]
    fn break_exits_while_loop() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga i = 0;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "i", 0),
            Some(Expr::Literal {
                value: Literal::Number(0.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // segueOFluxo (firmeza) { salve i; sePá (i == 2) saiFora; i = i + 1; }
        let while_stmt = Stmt::while_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            Stmt::block(vec![
                Stmt::print(Expr::Variable {
                    name: make_token(crate::token::TokenType::Identifier, "i", 0),
                }),
                Stmt::if_stmt(
                    Expr::Binary {
                        left: Box::new(Expr::Variable {
                            name: make_token(crate::token::TokenType::Identifier, "i", 0),
                        }),
                        operator: make_token(crate::token::TokenType::EqualEqual, "==", 0),
                        right: Box::new(Expr::Literal {
                            value: Literal::Number(2.0),
                        }),
                    },
                    Stmt::break_stmt(),
                    None,
                ),
                Stmt::expression(Expr::Assign {
                    name: make_token(crate::token::TokenType::Identifier, "i", 0),
                    value: Box::new(Expr::Binary {
                        left: Box::new(Expr::Variable {
                            name: make_token(crate::token::TokenType::Identifier, "i", 0),
                        }),
                        operator: make_token(crate::token::TokenType::Plus, "+", 0),
                        right: Box::new(Expr::Literal {
                            value: Literal::Number(1.0),
                        }),
                    }),
                }),
            ]),
        );
        interpreter.execute(&while_stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "0\n1\n2\n");
    }

    #[test]
    fn break_exits_immediately() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // segueOFluxo (firmeza) { salve 1; saiFora; salve 2; }
        let stmt = Stmt::while_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            Stmt::block(vec![
                Stmt::print(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                Stmt::break_stmt(),
                Stmt::print(Expr::Literal {
                    value: Literal::Number(2.0),
                }),
            ]),
        );
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn while_propagates_runtime_error() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // segueOFluxo (firmeza) { salve -"oops"; }
        let stmt = Stmt::while_stmt(
            Expr::Literal {
                value: Literal::Bool(true),
            },
            Stmt::print(Expr::Unary {
                operator: make_token(crate::token::TokenType::Minus, "-", 0),
                right: Box::new(Expr::Literal {
                    value: Literal::String("oops".to_string()),
                }),
            }),
        );
        let result = interpreter.execute(&stmt, &mut output);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === else statements ===

    #[test]
    fn executes_else_body() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        let stmt = Stmt::Else {
            body: Box::new(Stmt::print(Expr::Literal {
                value: Literal::Number(42.0),
            })),
            span: 0..10,
        };
        interpreter.execute(&stmt, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    // === function declarations ===

    #[test]
    fn function_declaration_defines_variable() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita cumprimentar() { salve 42; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "cumprimentar", 0),
            params: vec![],
            body: vec![Stmt::print(Expr::Literal {
                value: Literal::Number(42.0),
            })],
            span: 0..30,
        };
        interpreter.execute(&func_stmt, &mut output).unwrap();

        // Function should be defined
        assert!(
            interpreter
                .variable_names()
                .contains(&"cumprimentar".to_string())
        );
    }

    #[test]
    fn function_call_executes_body() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita cumprimentar() { salve 42; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "cumprimentar", 0),
            params: vec![],
            body: vec![Stmt::print(Expr::Literal {
                value: Literal::Number(42.0),
            })],
            span: 0..30,
        };
        interpreter.execute(&func_stmt, &mut output).unwrap();

        // cumprimentar();
        let call_stmt = Stmt::expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "cumprimentar", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        });
        interpreter.execute(&call_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "42\n");
    }

    #[test]
    fn function_call_with_arguments() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita saudar(nome) { salve nome; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "saudar", 0),
            params: vec![make_token(TokenType::Identifier, "nome", 0)],
            body: vec![Stmt::print(Expr::Variable {
                name: make_token(TokenType::Identifier, "nome", 0),
            })],
            span: 0..30,
        };
        interpreter.execute(&func_stmt, &mut output).unwrap();

        // saudar("mano");
        let call_stmt = Stmt::expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "saudar", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![Expr::Literal {
                value: Literal::String("mano".to_string()),
            }],
        });
        interpreter.execute(&call_stmt, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "mano\n");
    }

    #[test]
    fn function_call_wrong_arity_errors() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita soma(a, b) { salve a; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "soma", 0),
            params: vec![
                make_token(TokenType::Identifier, "a", 0),
                make_token(TokenType::Identifier, "b", 0),
            ],
            body: vec![],
            span: 0..30,
        };
        interpreter.execute(&func_stmt, &mut output).unwrap();

        // soma(1); -- wrong arity!
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "soma", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![Expr::Literal {
                value: Literal::Number(1.0),
            }],
        };
        let result = eval(&mut interpreter, &call_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn calling_non_function_errors() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 42;
        let var_stmt = Stmt::var(
            make_token(TokenType::Identifier, "x", 0),
            Some(Expr::Literal {
                value: Literal::Number(42.0),
            }),
        );
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // x(); -- can't call a number!
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "x", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        };
        let result = eval(&mut interpreter, &call_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn calling_undefined_function_errors() {
        let mut interpreter = Interpreter::new();

        // naoExiste(); -- function not defined!
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "naoExiste", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        };
        let result = eval(&mut interpreter, &call_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === native functions ===

    #[test]
    fn faz_teu_corre_returns_number() {
        let mut interpreter = Interpreter::new();

        // fazTeuCorre();
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "fazTeuCorre", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        };
        let result = eval(&mut interpreter, &call_expr).unwrap();
        assert!(matches!(result, Value::Literal(Literal::Number(_))));
    }

    #[test]
    fn faz_teu_corre_returns_increasing_time() {
        let mut interpreter = Interpreter::new();

        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "fazTeuCorre", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        };

        let first = eval(&mut interpreter, &call_expr).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let second = eval(&mut interpreter, &call_expr).unwrap();

        match (first, second) {
            (Value::Literal(Literal::Number(t1)), Value::Literal(Literal::Number(t2))) => {
                assert!(t2 > t1, "Time should increase");
            }
            _ => panic!("Expected numbers"),
        }
    }

    #[test]
    fn native_function_wrong_arity_errors() {
        let mut interpreter = Interpreter::new();

        // fazTeuCorre(42); -- wrong arity, expects 0 arguments!
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "fazTeuCorre", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![Expr::Literal {
                value: Literal::Number(42.0),
            }],
        };
        let result = eval(&mut interpreter, &call_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === return statements ===

    #[test]
    fn function_returns_value() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita resposta() { toma 42; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "resposta", 0),
            params: vec![],
            body: vec![Stmt::Return {
                keyword: make_token(TokenType::Return, "toma", 0),
                value: Some(Expr::Literal {
                    value: Literal::Number(42.0),
                }),
                span: 0..10,
            }],
            span: 0..30,
        };
        interpreter.execute(&func_stmt, &mut output).unwrap();

        // resposta();
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "resposta", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        };
        let result = eval(&mut interpreter, &call_expr).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn function_returns_nil_without_value() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita nada() { toma; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "nada", 0),
            params: vec![],
            body: vec![Stmt::Return {
                keyword: make_token(TokenType::Return, "toma", 0),
                value: None,
                span: 0..5,
            }],
            span: 0..20,
        };
        interpreter.execute(&func_stmt, &mut output).unwrap();

        // nada();
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "nada", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        };
        let result = eval(&mut interpreter, &call_expr).unwrap();
        assert_eq!(result, nil());
    }

    #[test]
    fn function_early_return() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita cedo() { toma 1; salve 2; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "cedo", 0),
            params: vec![],
            body: vec![
                Stmt::Return {
                    keyword: make_token(TokenType::Return, "toma", 0),
                    value: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..8,
                },
                Stmt::print(Expr::Literal {
                    value: Literal::Number(2.0),
                }),
            ],
            span: 0..30,
        };
        interpreter.execute(&func_stmt, &mut output).unwrap();

        // cedo();
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "cedo", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        };
        let result = eval(&mut interpreter, &call_expr).unwrap();
        assert_eq!(result, num(1.0));
        // salve 2 should NOT be printed
        assert!(output.is_empty());
    }

    #[test]
    fn lambda_creates_callable_value() {
        let mut interpreter = Interpreter::new();
        let lambda_expr = Expr::Lambda {
            params: vec![make_token(TokenType::Identifier, "x", 0)],
            body: vec![Stmt::Return {
                keyword: make_token(TokenType::Return, "toma", 0),
                value: Some(Expr::Binary {
                    left: Box::new(Expr::Variable {
                        name: make_token(TokenType::Identifier, "x", 0),
                    }),
                    operator: make_token(TokenType::Star, "*", 0),
                    right: Box::new(Expr::Literal {
                        value: Literal::Number(2.0),
                    }),
                }),
                span: 0..10,
            }],
        };
        let result = eval(&mut interpreter, &lambda_expr).unwrap();
        assert!(matches!(result, Value::Function(_)));
    }

    #[test]
    fn lambda_can_be_called() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga dobro = olhaEssaFita (x) { toma x * 2; };
        let var_stmt = Stmt::Var {
            name: make_token(TokenType::Identifier, "dobro", 0),
            initializer: Some(Expr::Lambda {
                params: vec![make_token(TokenType::Identifier, "x", 0)],
                body: vec![Stmt::Return {
                    keyword: make_token(TokenType::Return, "toma", 0),
                    value: Some(Expr::Binary {
                        left: Box::new(Expr::Variable {
                            name: make_token(TokenType::Identifier, "x", 0),
                        }),
                        operator: make_token(TokenType::Star, "*", 0),
                        right: Box::new(Expr::Literal {
                            value: Literal::Number(2.0),
                        }),
                    }),
                    span: 0..10,
                }],
            }),
            span: 0..30,
        };
        interpreter.execute(&var_stmt, &mut output).unwrap();

        // dobro(5);
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "dobro", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![Expr::Literal {
                value: Literal::Number(5.0),
            }],
        };
        let result = eval(&mut interpreter, &call_expr).unwrap();
        assert_eq!(result, num(10.0));
    }

    #[test]
    fn function_propagates_runtime_error() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita quebra() { toma 1 + "texto"; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "quebra", 0),
            params: vec![],
            body: vec![Stmt::Return {
                keyword: make_token(TokenType::Return, "toma", 0),
                value: Some(Expr::Binary {
                    left: Box::new(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    operator: make_token(TokenType::Plus, "+", 0),
                    right: Box::new(Expr::Literal {
                        value: Literal::String("texto".to_string()),
                    }),
                }),
                span: 0..15,
            }],
            span: 0..30,
        };
        interpreter.execute(&func_stmt, &mut output).unwrap();

        // quebra();
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "quebra", 0),
            }),
            paren: make_token(TokenType::RightParen, ")", 0),
            arguments: vec![],
        };
        let result = eval(&mut interpreter, &call_expr);
        assert!(result.is_err());
    }
}
