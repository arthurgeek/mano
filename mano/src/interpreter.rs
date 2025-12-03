use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;
use std::time::SystemTime;

use crate::INITIALIZER_NAME;
use crate::ast::{Expr, Stmt};
use crate::environment::Environment;
use crate::error::ManoError;
use crate::resolver::Resolutions;
use crate::token::{Literal, TokenType};
use crate::value::{Class, Function, Instance, ManoFunction, NativeFunction, Value};

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
    globals: Rc<RefCell<Environment>>,
    resolutions: Resolutions,
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

        Self {
            globals: Rc::clone(&environment),
            environment,
            resolutions: Resolutions::new(),
        }
    }

    pub fn variable_names(&self) -> Vec<String> {
        self.environment.borrow().variable_names()
    }

    pub fn set_resolutions(&mut self, resolutions: Resolutions) {
        self.resolutions = resolutions;
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
                // Use slot-based for locals, name-based for globals
                if Rc::ptr_eq(&self.environment, &self.globals) {
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
                } else if let Some(expr) = initializer {
                    let value = self.interpret(expr, output)?;
                    self.environment
                        .borrow_mut()
                        .define_at_slot(name.lexeme.clone(), value);
                } else {
                    self.environment
                        .borrow_mut()
                        .define_uninitialized_at_slot(name.lexeme.clone());
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
                name,
                params,
                body,
                is_getter,
                ..
            } => {
                let function = ManoFunction {
                    name: Some(name.clone()),
                    params: params.clone(),
                    body: body.clone(),
                    closure: Rc::clone(&self.environment),
                    is_getter: *is_getter,
                };
                let value = Value::Function(Rc::new(Function::Mano(function)));

                // Use slot-based for locals, name-based for globals
                if Rc::ptr_eq(&self.environment, &self.globals) {
                    self.environment
                        .borrow_mut()
                        .define(name.lexeme.clone(), value);
                } else {
                    self.environment
                        .borrow_mut()
                        .define_at_slot(name.lexeme.clone(), value);
                }
                Ok(())
            }
            Stmt::Return { value, .. } => {
                let return_value = match value {
                    Some(expr) => self.interpret(expr, output)?,
                    None => Value::Literal(Literal::Nil),
                };
                Err(ManoError::Return(return_value))
            }
            Stmt::Class { name, methods, .. } => {
                // Define class name (with nil initially)
                self.environment
                    .borrow_mut()
                    .define(name.lexeme.clone(), Value::Literal(Literal::Nil));

                // Create class (separate static and instance methods)
                let mut method_map = HashMap::new();
                let mut static_method_map = HashMap::new();
                for method in methods {
                    if let Stmt::Function {
                        name: method_name,
                        params,
                        body,
                        is_static,
                        is_getter,
                        ..
                    } = method
                    {
                        let function = ManoFunction {
                            name: Some(method_name.clone()),
                            params: params.clone(),
                            body: body.clone(),
                            closure: Rc::clone(&self.environment),
                            is_getter: *is_getter,
                        };
                        if *is_static {
                            static_method_map.insert(
                                method_name.lexeme.clone(),
                                Rc::new(Function::Mano(function)),
                            );
                        } else {
                            method_map.insert(
                                method_name.lexeme.clone(),
                                Rc::new(Function::Mano(function)),
                            );
                        }
                    }
                }
                let class = Class {
                    name: name.lexeme.clone(),
                    methods: method_map,
                    static_methods: static_method_map,
                };

                // Assign the class value
                self.environment.borrow_mut().assign(
                    &name.lexeme,
                    Value::Class(Rc::new(class)),
                    name.span.clone(),
                )?;
                Ok(())
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
            Expr::Variable { name } => {
                if let Some(&(distance, slot)) = self.resolutions.get(&name.span) {
                    self.environment
                        .borrow()
                        .get_at(distance, slot)
                        .ok_or_else(|| ManoError::Runtime {
                            message: format!("Variável '{}' não existe, mano!", name.lexeme),
                            span: name.span.clone(),
                        })
                } else {
                    // Unresolved = must be global
                    self.globals.borrow().get(&name.lexeme, name.span.clone())
                }
            }
            Expr::Assign { name, value } => {
                let val = self.interpret(value, output)?;
                if let Some(&(distance, slot)) = self.resolutions.get(&name.span) {
                    self.environment
                        .borrow_mut()
                        .assign_at(distance, slot, val.clone());
                } else {
                    // Unresolved = must be global
                    self.globals.borrow_mut().assign(
                        &name.lexeme,
                        val.clone(),
                        name.span.clone(),
                    )?;
                }
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
                                is_getter: mano_func.is_getter,
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
                    Value::Class(class) => {
                        // Create the instance first
                        let instance = Rc::new(Instance {
                            class: Rc::clone(&class),
                            fields: RefCell::new(HashMap::new()),
                        });

                        // Look for initializer (bora)
                        if let Some(initializer) = class.methods.get(INITIALIZER_NAME) {
                            match initializer.as_ref() {
                                Function::Mano(func) => {
                                    // Check arity
                                    if args.len() != func.params.len() {
                                        return Err(ManoError::Runtime {
                                            message: format!(
                                                "Esse bagulho espera {} lances, mas tu passou {}, mano!",
                                                func.params.len(),
                                                args.len()
                                            ),
                                            span: paren.span.clone(),
                                        });
                                    }
                                    // Bind and call bora
                                    let bound = func.bind(Rc::clone(&instance));
                                    self.call_mano_function(&bound, args, output)?;
                                }
                                Function::Native(_) => {
                                    // Native initializers shouldn't happen
                                }
                            }
                        } else {
                            // No bora, so class takes no arguments
                            if !args.is_empty() {
                                return Err(ManoError::Runtime {
                                    message: format!(
                                        "Esse bagulho espera 0 lances, mas tu passou {}, mano!",
                                        args.len()
                                    ),
                                    span: paren.span.clone(),
                                });
                            }
                        }

                        Ok(Value::Instance(instance))
                    }
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
                    is_getter: false,
                };
                Ok(Value::Function(Rc::new(Function::Mano(func))))
            }
            Expr::Get { object, name } => {
                let object_value = self.interpret(object, output)?;
                match object_value {
                    Value::Instance(instance) => {
                        // First check fields
                        if let Some(value) = instance.fields.borrow().get(&name.lexeme) {
                            return Ok(value.clone());
                        }

                        // Then check methods on the class (bind to instance)
                        // Note: static methods are NOT accessible on instances
                        let method = instance.class.methods.get(&name.lexeme).cloned();
                        if let Some(method) = method {
                            if let Function::Mano(func) = method.as_ref() {
                                let bound = func.bind(Rc::clone(&instance));
                                // If it's a getter, auto-invoke it
                                if func.is_getter {
                                    return self.call_mano_function(&bound, vec![], output);
                                }
                                return Ok(Value::Function(Rc::new(Function::Mano(bound))));
                            }
                            return Ok(Value::Function(method));
                        }

                        Err(ManoError::Runtime {
                            message: format!("Eita, '{}' não existe nessa parada!", name.lexeme),
                            span: name.span.clone(),
                        })
                    }
                    Value::Class(class) => {
                        // Static methods are accessible on class itself
                        if let Some(method) = class.static_methods.get(&name.lexeme).cloned() {
                            return Ok(Value::Function(method));
                        }

                        Err(ManoError::Runtime {
                            message: format!(
                                "Eita, '{}' não é fita estática do bagulho {}!",
                                name.lexeme, class.name
                            ),
                            span: name.span.clone(),
                        })
                    }
                    _ => Err(ManoError::Runtime {
                        message: "Só parada tem esquema, chapa!".to_string(),
                        span: name.span.clone(),
                    }),
                }
            }
            Expr::Set {
                object,
                name,
                value,
            } => {
                let object_value = self.interpret(object, output)?;
                match object_value {
                    Value::Instance(instance) => {
                        let val = self.interpret(value, output)?;
                        instance
                            .fields
                            .borrow_mut()
                            .insert(name.lexeme.clone(), val.clone());
                        Ok(val)
                    }
                    _ => Err(ManoError::Runtime {
                        message: "Só parada tem esquema, chapa!".to_string(),
                        span: name.span.clone(),
                    }),
                }
            }
            Expr::This { keyword } => {
                // Look up "oCara" using resolution - same as Variable
                if let Some(&(distance, slot)) = self.resolutions.get(&keyword.span) {
                    self.environment
                        .borrow()
                        .get_at(distance, slot)
                        .ok_or_else(|| ManoError::Runtime {
                            message: "oCara não existe, mano!".to_string(),
                            span: keyword.span.clone(),
                        })
                } else {
                    // Unresolved = must be global (shouldn't happen for oCara)
                    self.globals.borrow().get("oCara", keyword.span.clone())
                }
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

        // Bind parameters to arguments (function scope is always local, use slots)
        for (param, arg) in func.params.iter().zip(args.into_iter()) {
            self.environment
                .borrow_mut()
                .define_at_slot(param.lexeme.clone(), arg);
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
    use crate::token::Token;

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
    fn local_uninitialized_variable_uses_slot() {
        // { seLiga x; x = 42; salve x; }
        let mut interpreter = Interpreter::new();
        interpreter.set_resolutions(
            [(0..1, (0, 0)), (10..11, (0, 0)), (20..21, (0, 0))]
                .into_iter()
                .collect(),
        );
        let mut output = Vec::new();

        let block = Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: crate::token::Token {
                        token_type: crate::token::TokenType::Identifier,
                        lexeme: "x".to_string(),
                        literal: None,
                        span: 0..1,
                    },
                    initializer: None,
                    span: 0..5,
                },
                Stmt::Expression {
                    expression: Expr::Assign {
                        name: crate::token::Token {
                            token_type: crate::token::TokenType::Identifier,
                            lexeme: "x".to_string(),
                            literal: None,
                            span: 10..11,
                        },
                        value: Box::new(Expr::Literal {
                            value: crate::token::Literal::Number(42.0),
                        }),
                    },
                    span: 10..15,
                },
                Stmt::Print {
                    expression: Expr::Variable {
                        name: crate::token::Token {
                            token_type: crate::token::TokenType::Identifier,
                            lexeme: "x".to_string(),
                            literal: None,
                            span: 20..21,
                        },
                    },
                    span: 20..25,
                },
            ],
            span: 0..30,
        };

        interpreter.execute(&block, &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap().trim(), "42");
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
        use crate::resolver::Resolver;
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga x = 1;
        let var_stmt = Stmt::var(
            make_token(crate::token::TokenType::Identifier, "x", 1),
            Some(Expr::Literal {
                value: Literal::Number(1.0),
            }),
        );

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

        // salve x; (should be 1 again)
        let print_stmt = Stmt::print(Expr::Variable {
            name: make_token(crate::token::TokenType::Identifier, "x", 4),
        });

        // Resolve all statements together
        let statements = vec![var_stmt.clone(), block.clone(), print_stmt.clone()];
        let resolver = Resolver::new();
        let resolutions = resolver.resolve(&statements).unwrap();
        interpreter.set_resolutions(resolutions);

        // Execute
        interpreter.execute(&var_stmt, &mut output).unwrap();
        interpreter.execute(&block, &mut output).unwrap();
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
            is_static: false,
            is_getter: false,
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
            is_static: false,
            is_getter: false,
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
        use crate::resolver::Resolver;
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // olhaEssaFita saudar(nome) { salve nome; }
        let func_stmt = Stmt::Function {
            name: make_token(TokenType::Identifier, "saudar", 0),
            params: vec![make_token(TokenType::Identifier, "nome", 10)],
            body: vec![Stmt::print(Expr::Variable {
                name: make_token(TokenType::Identifier, "nome", 20),
            })],
            is_static: false,
            is_getter: false,
            span: 0..30,
        };

        // saudar("mano");
        let call_stmt = Stmt::expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "saudar", 40),
            }),
            paren: make_token(TokenType::RightParen, ")", 50),
            arguments: vec![Expr::Literal {
                value: Literal::String("mano".to_string()),
            }],
        });

        // Resolve
        let statements = vec![func_stmt.clone(), call_stmt.clone()];
        let resolver = Resolver::new();
        let resolutions = resolver.resolve(&statements).unwrap();
        interpreter.set_resolutions(resolutions);

        // Execute
        interpreter.execute(&func_stmt, &mut output).unwrap();
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
            is_static: false,
            is_getter: false,
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
            is_static: false,
            is_getter: false,
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
            is_static: false,
            is_getter: false,
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
            is_static: false,
            is_getter: false,
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
        use crate::resolver::Resolver;
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // seLiga dobro = olhaEssaFita (x) { toma x * 2; };
        let var_stmt = Stmt::Var {
            name: make_token(TokenType::Identifier, "dobro", 0),
            initializer: Some(Expr::Lambda {
                params: vec![make_token(TokenType::Identifier, "x", 10)],
                body: vec![Stmt::Return {
                    keyword: make_token(TokenType::Return, "toma", 20),
                    value: Some(Expr::Binary {
                        left: Box::new(Expr::Variable {
                            name: make_token(TokenType::Identifier, "x", 30),
                        }),
                        operator: make_token(TokenType::Star, "*", 35),
                        right: Box::new(Expr::Literal {
                            value: Literal::Number(2.0),
                        }),
                    }),
                    span: 20..40,
                }],
            }),
            span: 0..50,
        };

        // dobro(5);
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: make_token(TokenType::Identifier, "dobro", 60),
            }),
            paren: make_token(TokenType::RightParen, ")", 70),
            arguments: vec![Expr::Literal {
                value: Literal::Number(5.0),
            }],
        };

        // Resolve
        let statements = vec![var_stmt.clone()];
        let resolver = Resolver::new();
        let resolutions = resolver.resolve(&statements).unwrap();
        interpreter.set_resolutions(resolutions);

        // Execute
        interpreter.execute(&var_stmt, &mut output).unwrap();
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
            is_static: false,
            is_getter: false,
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

    // === resolution tests ===

    #[test]
    fn set_resolutions_stores_resolved_distances() {
        use crate::resolver::Resolutions;
        let mut interpreter = Interpreter::new();
        let mut resolutions = Resolutions::new();
        resolutions.insert(10..15, (0, 0)); // span 10..15 resolves to distance 0
        interpreter.set_resolutions(resolutions.clone());
        // Just testing the method exists and stores the value
    }

    #[test]
    fn variable_uses_resolved_distance() {
        use crate::resolver::Resolutions;
        let mut interpreter = Interpreter::new();

        // Define x=42 in outer scope at slot 0
        let outer_env = Rc::clone(&interpreter.environment);
        outer_env
            .borrow_mut()
            .define_at_slot("x".to_string(), num(42.0)); // slot 0

        // Create inner scope and shadow x=99 at slot 0
        let inner_env = Rc::new(RefCell::new(Environment::with_enclosing(Rc::clone(
            &outer_env,
        ))));
        inner_env
            .borrow_mut()
            .define_at_slot("x".to_string(), num(99.0)); // slot 0
        interpreter.environment = inner_env;

        // Variable expression at span 0..1
        let var_expr = Expr::Variable {
            name: make_token(TokenType::Identifier, "x", 0),
        };

        // Set resolution: span 0..1 should resolve to distance 1, slot 0 (outer x=42)
        let mut resolutions = Resolutions::new();
        resolutions.insert(0..1, (1, 0));
        interpreter.set_resolutions(resolutions);

        // Should find x=42 at distance 1, NOT x=99 at distance 0
        let result = eval(&mut interpreter, &var_expr).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn unresolved_assign_updates_globals_only() {
        use crate::resolver::Resolutions;
        let mut interpreter = Interpreter::new();

        // Define x=1 in globals
        interpreter
            .globals
            .borrow_mut()
            .define("x".to_string(), num(1.0));

        // Create a non-global scope with local x=50 at slot 0
        let inner_env = Rc::new(RefCell::new(Environment::with_enclosing(Rc::clone(
            &interpreter.globals,
        ))));
        inner_env
            .borrow_mut()
            .define_at_slot("x".to_string(), num(50.0)); // slot 0
        interpreter.environment = Rc::clone(&inner_env);

        // Assign expression - NO resolution (global variable)
        let assign_expr = Expr::Assign {
            name: make_token(TokenType::Identifier, "x", 0),
            value: Box::new(Expr::Literal {
                value: Literal::Number(99.0),
            }),
        };

        // No resolutions set - should assign to globals ONLY
        interpreter.set_resolutions(Resolutions::new());
        eval(&mut interpreter, &assign_expr).unwrap();

        // Global x should be updated to 99
        let global_result = interpreter.globals.borrow().get("x", 0..1).unwrap();
        assert_eq!(global_result, num(99.0));

        // Local slot 0 should still be 50
        let local_result = inner_env.borrow().get_at(0, 0).unwrap();
        assert_eq!(local_result, num(50.0));
    }

    #[test]
    fn unresolved_variable_looks_up_in_globals_only() {
        use crate::resolver::Resolutions;
        let mut interpreter = Interpreter::new();

        // Define x=42 in globals
        interpreter
            .globals
            .borrow_mut()
            .define("x".to_string(), num(42.0));

        // Create a non-global scope that shadows x=99
        let inner_env = Rc::new(RefCell::new(Environment::with_enclosing(Rc::clone(
            &interpreter.globals,
        ))));
        inner_env.borrow_mut().define("x".to_string(), num(99.0));
        interpreter.environment = inner_env;

        // Variable expression at span 0..1 - NO resolution (global variable)
        let var_expr = Expr::Variable {
            name: make_token(TokenType::Identifier, "x", 0),
        };

        // No resolutions set - should look up in globals ONLY, finding x=42
        interpreter.set_resolutions(Resolutions::new());

        let result = eval(&mut interpreter, &var_expr).unwrap();
        // Should find global x=42, NOT the shadowing x=99
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn resolved_variable_not_found_returns_error() {
        use crate::resolver::Resolutions;
        let mut interpreter = Interpreter::new();

        // Variable expression at span 0..1
        let var_expr = Expr::Variable {
            name: make_token(TokenType::Identifier, "x", 0),
        };

        // Set resolution saying x is at distance 0, but don't define x
        let mut resolutions = Resolutions::new();
        resolutions.insert(0..1, (0, 0));
        interpreter.set_resolutions(resolutions);

        // Should error because x doesn't exist at the resolved distance
        let result = eval(&mut interpreter, &var_expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
        if let Err(ManoError::Runtime { message, .. }) = result {
            assert!(message.contains("não existe"));
        }
    }

    #[test]
    fn assign_uses_resolved_distance() {
        use crate::resolver::Resolutions;
        let mut interpreter = Interpreter::new();

        // Define x=1 in outer scope at slot 0
        let outer_env = Rc::clone(&interpreter.environment);
        outer_env
            .borrow_mut()
            .define_at_slot("x".to_string(), num(1.0)); // slot 0

        // Create inner scope and shadow x=50 at slot 0
        let inner_env = Rc::new(RefCell::new(Environment::with_enclosing(Rc::clone(
            &outer_env,
        ))));
        inner_env
            .borrow_mut()
            .define_at_slot("x".to_string(), num(50.0)); // slot 0
        interpreter.environment = Rc::clone(&inner_env);

        // Assign expression at span 0..5 assigns to x
        let assign_expr = Expr::Assign {
            name: make_token(TokenType::Identifier, "x", 0),
            value: Box::new(Expr::Literal {
                value: Literal::Number(99.0),
            }),
        };

        // Set resolution: span 0..1 resolves to distance 1, slot 0 (outer scope)
        let mut resolutions = Resolutions::new();
        resolutions.insert(0..1, (1, 0));
        interpreter.set_resolutions(resolutions);

        // Execute assignment
        eval(&mut interpreter, &assign_expr).unwrap();

        // Check that outer slot 0 was updated to 99
        let outer_result = outer_env.borrow().get_at(0, 0).unwrap();
        assert_eq!(outer_result, num(99.0));

        // And local slot 0 should still be 50
        let local_result = inner_env.borrow().get_at(0, 0).unwrap();
        assert_eq!(local_result, num(50.0));
    }

    #[test]
    fn class_declaration_stores_class_in_environment() {
        let mut interpreter = Interpreter::new();
        let stmt = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 8..14,
            },
            methods: vec![],
            span: 0..17,
        };

        let mut output = Vec::new();
        interpreter.execute(&stmt, &mut output).unwrap();

        // Class should be stored in global environment
        let names = interpreter.variable_names();
        assert!(names.contains(&"Pessoa".to_string()));
    }

    #[test]
    fn class_value_displays_as_bagulho() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // Declare class
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Vazio".to_string(),
                literal: None,
                span: 8..13,
            },
            methods: vec![],
            span: 0..16,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // Print class
        let print_stmt = Stmt::Print {
            expression: Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Vazio".to_string(),
                    literal: None,
                    span: 22..27,
                },
            },
            span: 17..28,
        };

        output.clear();
        interpreter.execute(&print_stmt, &mut output).unwrap();

        let printed = String::from_utf8(output).unwrap();
        assert_eq!(printed.trim(), "<bagulho Vazio>");
    }

    #[test]
    fn class_stores_methods() {
        let mut interpreter = Interpreter::new();
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 8..14,
            },
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "falar".to_string(),
                    literal: None,
                    span: 17..22,
                },
                params: vec![],
                body: vec![],
                is_static: false,
                is_getter: false,
                span: 17..30,
            }],
            span: 0..32,
        };

        let mut output = Vec::new();
        interpreter.execute(&class_decl, &mut output).unwrap();

        // Just verify it doesn't error - we can't inspect methods easily
        // In later sections we'll test calling them
        let names = interpreter.variable_names();
        assert!(names.contains(&"Pessoa".to_string()));
    }

    #[test]
    fn calling_class_creates_instance() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // bagulho Pessoa {}
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 0..6,
            },
            methods: vec![],
            span: 0..20,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // Pessoa()
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Pessoa".to_string(),
                    literal: None,
                    span: 0..6,
                },
            }),
            paren: Token {
                token_type: TokenType::RightParen,
                lexeme: ")".to_string(),
                literal: None,
                span: 7..8,
            },
            arguments: vec![],
        };

        let result = interpreter.interpret(&call_expr, &mut output).unwrap();
        assert!(matches!(result, Value::Instance(_)));
    }

    #[test]
    fn calling_class_with_arguments_errors() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // bagulho Pessoa {}
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 0..6,
            },
            methods: vec![],
            span: 0..20,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // Pessoa(1, 2)
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Pessoa".to_string(),
                    literal: None,
                    span: 0..6,
                },
            }),
            paren: Token {
                token_type: TokenType::RightParen,
                lexeme: ")".to_string(),
                literal: None,
                span: 10..11,
            },
            arguments: vec![
                Expr::Literal {
                    value: Literal::Number(1.0),
                },
                Expr::Literal {
                    value: Literal::Number(2.0),
                },
            ],
        };

        let result = interpreter.interpret(&call_expr, &mut output);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
        if let Err(ManoError::Runtime { message, .. }) = result {
            assert!(message.contains("0 lances"));
            assert!(message.contains("2"));
        }
    }

    // === get/set expressions ===

    #[test]
    fn get_on_non_instance_errors() {
        let mut interpreter = Interpreter::new();
        // Try to access .nome on a number (not an instance)
        let expr = Expr::Get {
            object: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "nome".to_string(),
                literal: None,
                span: 3..7,
            },
        };
        let result = eval(&mut interpreter, &expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn set_field_on_instance() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // bagulho Pessoa {}
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 0..6,
            },
            methods: vec![],
            span: 0..20,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // seLiga p = Pessoa();
        let var_decl = Stmt::Var {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "p".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: Some(Expr::Call {
                callee: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Pessoa".to_string(),
                        literal: None,
                        span: 0..6,
                    },
                }),
                paren: Token {
                    token_type: TokenType::RightParen,
                    lexeme: ")".to_string(),
                    literal: None,
                    span: 7..8,
                },
                arguments: vec![],
            }),
            span: 0..10,
        };
        interpreter.execute(&var_decl, &mut output).unwrap();

        // p.nome = "João" should return "João"
        let set_expr = Expr::Set {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "p".to_string(),
                    literal: None,
                    span: 0..1,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "nome".to_string(),
                literal: None,
                span: 2..6,
            },
            value: Box::new(Expr::Literal {
                value: Literal::String("João".to_string()),
            }),
        };
        let result = interpreter.interpret(&set_expr, &mut output).unwrap();
        assert_eq!(result, str("João"));
    }

    #[test]
    fn set_on_non_instance_errors() {
        let mut interpreter = Interpreter::new();
        // Try to set .nome on a number (not an instance)
        let expr = Expr::Set {
            object: Box::new(Expr::Literal {
                value: Literal::Number(42.0),
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "nome".to_string(),
                literal: None,
                span: 3..7,
            },
            value: Box::new(Expr::Literal {
                value: Literal::String("João".to_string()),
            }),
        };
        let result = eval(&mut interpreter, &expr);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn get_undefined_property_errors() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // bagulho Pessoa {}
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 0..6,
            },
            methods: vec![],
            span: 0..20,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // seLiga p = Pessoa();
        let var_decl = Stmt::Var {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "p".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: Some(Expr::Call {
                callee: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Pessoa".to_string(),
                        literal: None,
                        span: 0..6,
                    },
                }),
                paren: Token {
                    token_type: TokenType::RightParen,
                    lexeme: ")".to_string(),
                    literal: None,
                    span: 7..8,
                },
                arguments: vec![],
            }),
            span: 0..10,
        };
        interpreter.execute(&var_decl, &mut output).unwrap();

        // p.undefined should error
        let get_expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "p".to_string(),
                    literal: None,
                    span: 0..1,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "undefined".to_string(),
                literal: None,
                span: 2..11,
            },
        };
        let result = interpreter.interpret(&get_expr, &mut output);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn get_existing_field_from_instance() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // bagulho Pessoa {}
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 0..6,
            },
            methods: vec![],
            span: 0..20,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // seLiga p = Pessoa();
        let var_decl = Stmt::Var {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "p".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: Some(Expr::Call {
                callee: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Pessoa".to_string(),
                        literal: None,
                        span: 0..6,
                    },
                }),
                paren: Token {
                    token_type: TokenType::RightParen,
                    lexeme: ")".to_string(),
                    literal: None,
                    span: 7..8,
                },
                arguments: vec![],
            }),
            span: 0..10,
        };
        interpreter.execute(&var_decl, &mut output).unwrap();

        // p.nome = "João";
        let set_expr = Expr::Set {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "p".to_string(),
                    literal: None,
                    span: 0..1,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "nome".to_string(),
                literal: None,
                span: 2..6,
            },
            value: Box::new(Expr::Literal {
                value: Literal::String("João".to_string()),
            }),
        };
        interpreter.interpret(&set_expr, &mut output).unwrap();

        // p.nome should return "João"
        let get_expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "p".to_string(),
                    literal: None,
                    span: 0..1,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "nome".to_string(),
                literal: None,
                span: 2..6,
            },
        };
        let result = interpreter.interpret(&get_expr, &mut output).unwrap();
        assert_eq!(result, str("João"));
    }

    #[test]
    fn get_method_from_instance() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // bagulho Pessoa { falar() { toma "oi"; } }
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 0..6,
            },
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "falar".to_string(),
                    literal: None,
                    span: 10..15,
                },
                params: vec![],
                body: vec![Stmt::Return {
                    keyword: Token {
                        token_type: TokenType::Return,
                        lexeme: "toma".to_string(),
                        literal: None,
                        span: 20..24,
                    },
                    value: Some(Expr::Literal {
                        value: Literal::String("oi".to_string()),
                    }),
                    span: 20..30,
                }],
                is_static: false,
                is_getter: false,
                span: 10..35,
            }],
            span: 0..40,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // seLiga p = Pessoa();
        let var_decl = Stmt::Var {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "p".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: Some(Expr::Call {
                callee: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Pessoa".to_string(),
                        literal: None,
                        span: 0..6,
                    },
                }),
                paren: Token {
                    token_type: TokenType::RightParen,
                    lexeme: ")".to_string(),
                    literal: None,
                    span: 7..8,
                },
                arguments: vec![],
            }),
            span: 0..10,
        };
        interpreter.execute(&var_decl, &mut output).unwrap();

        // p.falar should return the method (a function)
        let get_expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "p".to_string(),
                    literal: None,
                    span: 0..1,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "falar".to_string(),
                literal: None,
                span: 2..7,
            },
        };
        let result = interpreter.interpret(&get_expr, &mut output).unwrap();
        assert!(matches!(result, Value::Function(_)));
    }

    #[test]
    fn get_method_binds_o_cara() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // bagulho Pessoa { falar() {} }
        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 0..6,
            },
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "falar".to_string(),
                    literal: None,
                    span: 10..15,
                },
                params: vec![],
                body: vec![],
                is_static: false,
                is_getter: false,
                span: 10..20,
            }],
            span: 0..25,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // seLiga p = Pessoa();
        let var_decl = Stmt::Var {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "p".to_string(),
                literal: None,
                span: 0..1,
            },
            initializer: Some(Expr::Call {
                callee: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Pessoa".to_string(),
                        literal: None,
                        span: 0..6,
                    },
                }),
                paren: Token {
                    token_type: TokenType::RightParen,
                    lexeme: ")".to_string(),
                    literal: None,
                    span: 7..8,
                },
                arguments: vec![],
            }),
            span: 0..10,
        };
        interpreter.execute(&var_decl, &mut output).unwrap();

        // p.falar should return a bound method with oCara
        let get_expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "p".to_string(),
                    literal: None,
                    span: 0..1,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "falar".to_string(),
                literal: None,
                span: 2..7,
            },
        };
        let result = interpreter.interpret(&get_expr, &mut output).unwrap();

        // Check that it's a function with oCara bound at slot 0
        if let Value::Function(func) = result {
            if let Function::Mano(mano_func) = func.as_ref() {
                let o_cara = mano_func.closure.borrow().get_at(0, 0);
                assert!(
                    o_cara.is_some(),
                    "oCara should be defined in method closure"
                );
            } else {
                panic!("Expected ManoFunction");
            }
        } else {
            panic!("Expected Function");
        }
    }

    #[test]
    fn this_expression_returns_o_cara_value() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // Create an instance to be "oCara"
        let class = Rc::new(Class {
            name: "Pessoa".to_string(),
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        });
        let instance = Rc::new(Instance {
            class: Rc::clone(&class),
            fields: RefCell::new(HashMap::new()),
        });

        // Set up environment with oCara defined at slot 0
        interpreter
            .environment
            .borrow_mut()
            .define_at_slot("oCara".to_string(), Value::Instance(Rc::clone(&instance)));

        // Set up resolution for the This expression (distance 0, slot 0)
        let this_expr = Expr::This {
            keyword: Token {
                token_type: TokenType::This,
                lexeme: "oCara".to_string(),
                literal: None,
                span: 0..5,
            },
        };
        interpreter.set_resolutions([(0..5, (0, 0))].into_iter().collect());

        let result = interpreter.interpret(&this_expr, &mut output).unwrap();
        assert!(matches!(result, Value::Instance(_)));
    }

    #[test]
    fn this_unresolved_falls_back_to_globals() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // Create instance and put in globals as "oCara"
        let class = Rc::new(Class {
            name: "Test".to_string(),
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        });
        let instance = Rc::new(Instance {
            class: Rc::clone(&class),
            fields: RefCell::new(HashMap::new()),
        });
        interpreter
            .globals
            .borrow_mut()
            .define("oCara".to_string(), Value::Instance(Rc::clone(&instance)));

        // This expression WITHOUT resolution (empty resolutions map)
        let this_expr = Expr::This {
            keyword: Token {
                token_type: TokenType::This,
                lexeme: "oCara".to_string(),
                literal: None,
                span: 100..105, // Different span, not in resolutions
            },
        };
        // Don't set resolutions - this triggers the fallback path (lines 505-508)

        let result = interpreter.interpret(&this_expr, &mut output).unwrap();
        assert!(matches!(result, Value::Instance(_)));
    }

    #[test]
    fn this_resolved_but_slot_empty_returns_error() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // This expression with resolution set, but NO value defined at that slot
        let this_expr = Expr::This {
            keyword: Token {
                token_type: TokenType::This,
                lexeme: "oCara".to_string(),
                literal: None,
                span: 0..5,
            },
        };

        // Set up resolution pointing to slot 0, but DON'T define anything there
        // This triggers lines 502-503 (get_at returns None)
        interpreter.set_resolutions([(0..5, (0, 0))].into_iter().collect());

        let result = interpreter.interpret(&this_expr, &mut output);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            crate::error::ManoError::Runtime { message, .. } => {
                assert!(message.contains("oCara não existe"));
            }
            _ => panic!("Expected runtime error"),
        }
    }

    #[test]
    fn get_native_method_returns_without_binding() {
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // Create a class with a native function as method
        let native_fn = Rc::new(Function::Native(NativeFunction {
            name: "nativeMethod".to_string(),
            arity: 0,
            func: |_| Ok(Value::Literal(Literal::Number(42.0))),
        }));

        let mut methods = HashMap::new();
        methods.insert("nativeMethod".to_string(), native_fn);

        let class = Rc::new(Class {
            name: "TestClass".to_string(),
            methods,
            static_methods: HashMap::new(),
        });
        let instance = Rc::new(Instance {
            class: Rc::clone(&class),
            fields: RefCell::new(HashMap::new()),
        });

        // Manually set up the object to be the instance using slot-based storage
        interpreter
            .environment
            .borrow_mut()
            .define_at_slot("obj".to_string(), Value::Instance(Rc::clone(&instance)));

        let get_expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "obj".to_string(),
                    literal: None,
                    span: 0..3,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "nativeMethod".to_string(),
                literal: None,
                span: 4..16,
            },
        };

        // Set up resolution for the variable (distance 0, slot 0)
        interpreter.set_resolutions([(0..3, (0, 0))].into_iter().collect());

        let result = interpreter.interpret(&get_expr, &mut output).unwrap();
        // Should return the native function (line 460)
        assert!(matches!(result, Value::Function(_)));
    }

    #[test]
    fn native_initializer_is_silently_ignored() {
        // Edge case: class with a native function as "bora" initializer
        // This shouldn't happen in practice, but the code handles it gracefully
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        // Create a class with a native function named "bora"
        let native_bora = NativeFunction {
            name: "bora".to_string(),
            arity: 0,
            func: |_| Ok(Value::Literal(Literal::Nil)),
        };

        let mut methods = HashMap::new();
        methods.insert("bora".to_string(), Rc::new(Function::Native(native_bora)));

        let class = Rc::new(Class {
            name: "TestClass".to_string(),
            methods,
            static_methods: HashMap::new(),
        });

        // Store the class in environment
        interpreter
            .environment
            .borrow_mut()
            .define("TestClass".to_string(), Value::Class(class));

        // Call TestClass() - should create instance, native bora is ignored
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "TestClass".to_string(),
                    literal: None,
                    span: 0..9,
                },
            }),
            paren: Token {
                token_type: TokenType::RightParen,
                lexeme: ")".to_string(),
                literal: None,
                span: 10..11,
            },
            arguments: vec![],
        };

        let result = interpreter.interpret(&call_expr, &mut output);
        assert!(
            result.is_ok(),
            "Native initializer should be silently ignored"
        );
        assert!(matches!(result.unwrap(), Value::Instance(_)));
    }

    // === static methods ===

    #[test]
    fn static_method_callable_on_class() {
        // bagulho Math { bagulho soma(a, b) { toma a + b; } }
        // Math.soma(1, 2) -> 3
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Math".to_string(),
                literal: None,
                span: 0..4,
            },
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "soma".to_string(),
                    literal: None,
                    span: 10..14,
                },
                params: vec![
                    Token {
                        token_type: TokenType::Identifier,
                        lexeme: "a".to_string(),
                        literal: None,
                        span: 15..16,
                    },
                    Token {
                        token_type: TokenType::Identifier,
                        lexeme: "b".to_string(),
                        literal: None,
                        span: 18..19,
                    },
                ],
                body: vec![Stmt::Return {
                    keyword: Token {
                        token_type: TokenType::Return,
                        lexeme: "toma".to_string(),
                        literal: None,
                        span: 25..29,
                    },
                    value: Some(Expr::Binary {
                        left: Box::new(Expr::Variable {
                            name: Token {
                                token_type: TokenType::Identifier,
                                lexeme: "a".to_string(),
                                literal: None,
                                span: 30..31,
                            },
                        }),
                        operator: Token {
                            token_type: TokenType::Plus,
                            lexeme: "+".to_string(),
                            literal: None,
                            span: 32..33,
                        },
                        right: Box::new(Expr::Variable {
                            name: Token {
                                token_type: TokenType::Identifier,
                                lexeme: "b".to_string(),
                                literal: None,
                                span: 34..35,
                            },
                        }),
                    }),
                    span: 25..36,
                }],
                is_static: true,
                is_getter: false,
                span: 10..40,
            }],
            span: 0..45,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // Set up resolutions for params a and b (distance 0, slots 0 and 1)
        interpreter.set_resolutions([(30..31, (0, 0)), (34..35, (0, 1))].into_iter().collect());

        // Math.soma(1, 2)
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Get {
                object: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Math".to_string(),
                        literal: None,
                        span: 50..54,
                    },
                }),
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "soma".to_string(),
                    literal: None,
                    span: 55..59,
                },
            }),
            paren: Token {
                token_type: TokenType::RightParen,
                lexeme: ")".to_string(),
                literal: None,
                span: 65..66,
            },
            arguments: vec![
                Expr::Literal {
                    value: Literal::Number(1.0),
                },
                Expr::Literal {
                    value: Literal::Number(2.0),
                },
            ],
        };

        let result = interpreter.interpret(&call_expr, &mut output).unwrap();
        assert_eq!(result, Value::Literal(Literal::Number(3.0)));
    }

    #[test]
    fn static_method_not_callable_on_instance() {
        // bagulho Math { bagulho soma() {} }
        // seLiga m = Math();
        // m.soma() -> error
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Math".to_string(),
                literal: None,
                span: 0..4,
            },
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "soma".to_string(),
                    literal: None,
                    span: 10..14,
                },
                params: vec![],
                body: vec![],
                is_static: true,
                is_getter: false,
                span: 10..20,
            }],
            span: 0..25,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // seLiga m = Math();
        let var_decl = Stmt::Var {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "m".to_string(),
                literal: None,
                span: 30..31,
            },
            initializer: Some(Expr::Call {
                callee: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Math".to_string(),
                        literal: None,
                        span: 34..38,
                    },
                }),
                paren: Token {
                    token_type: TokenType::RightParen,
                    lexeme: ")".to_string(),
                    literal: None,
                    span: 40..41,
                },
                arguments: vec![],
            }),
            span: 30..42,
        };
        interpreter.execute(&var_decl, &mut output).unwrap();

        // m.soma()
        let call_expr = Expr::Call {
            callee: Box::new(Expr::Get {
                object: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "m".to_string(),
                        literal: None,
                        span: 50..51,
                    },
                }),
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "soma".to_string(),
                    literal: None,
                    span: 52..56,
                },
            }),
            paren: Token {
                token_type: TokenType::RightParen,
                lexeme: ")".to_string(),
                literal: None,
                span: 58..59,
            },
            arguments: vec![],
        };

        let result = interpreter.interpret(&call_expr, &mut output);
        assert!(
            result.is_err(),
            "static method should not be callable on instance"
        );
    }

    // === getter methods ===

    #[test]
    fn getter_auto_invoked_on_access() {
        // bagulho Pessoa { idade { toma 42; } }
        // seLiga p = Pessoa();
        // p.idade -> 42 (auto-invoked, no parentheses needed)
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 0..6,
            },
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "idade".to_string(),
                    literal: None,
                    span: 10..15,
                },
                params: vec![],
                body: vec![Stmt::Return {
                    keyword: Token {
                        token_type: TokenType::Return,
                        lexeme: "toma".to_string(),
                        literal: None,
                        span: 20..24,
                    },
                    value: Some(Expr::Literal {
                        value: Literal::Number(42.0),
                    }),
                    span: 20..27,
                }],
                is_static: false,
                is_getter: true,
                span: 10..30,
            }],
            span: 0..35,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // seLiga p = Pessoa();
        let var_decl = Stmt::Var {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "p".to_string(),
                literal: None,
                span: 40..41,
            },
            initializer: Some(Expr::Call {
                callee: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Pessoa".to_string(),
                        literal: None,
                        span: 44..50,
                    },
                }),
                paren: Token {
                    token_type: TokenType::RightParen,
                    lexeme: ")".to_string(),
                    literal: None,
                    span: 52..53,
                },
                arguments: vec![],
            }),
            span: 40..54,
        };
        interpreter.execute(&var_decl, &mut output).unwrap();

        // p.idade -> auto-invoked getter
        let get_expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "p".to_string(),
                    literal: None,
                    span: 60..61,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "idade".to_string(),
                literal: None,
                span: 62..67,
            },
        };

        let result = interpreter.interpret(&get_expr, &mut output).unwrap();
        assert_eq!(result, Value::Literal(Literal::Number(42.0)));
    }

    #[test]
    fn accessing_nonexistent_static_method_errors() {
        // bagulho Math { bagulho soma() {} }
        // Math.multiplica -> error (doesn't exist)
        let mut interpreter = Interpreter::new();
        let mut output = Vec::new();

        let class_decl = Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Math".to_string(),
                literal: None,
                span: 0..4,
            },
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "soma".to_string(),
                    literal: None,
                    span: 10..14,
                },
                params: vec![],
                body: vec![],
                is_static: true,
                is_getter: false,
                span: 10..20,
            }],
            span: 0..25,
        };
        interpreter.execute(&class_decl, &mut output).unwrap();

        // Math.multiplica -> error
        let get_expr = Expr::Get {
            object: Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Math".to_string(),
                    literal: None,
                    span: 30..34,
                },
            }),
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "multiplica".to_string(),
                literal: None,
                span: 35..45,
            },
        };

        let result = interpreter.interpret(&get_expr, &mut output);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
        if let Err(ManoError::Runtime { message, .. }) = result {
            assert!(message.contains("multiplica"));
            assert!(message.contains("Math"));
        }
    }
}
