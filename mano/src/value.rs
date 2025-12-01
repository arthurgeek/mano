use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::ast::Stmt;
use crate::environment::Environment;
use crate::token::{Literal, Token};

#[derive(Debug, Clone)]
pub enum Value {
    Literal(Literal),
    Function(Rc<Function>),
}

#[derive(Debug)]
pub enum Function {
    Mano(ManoFunction),
    Native(NativeFunction),
}

pub struct NativeFunction {
    pub name: String,
    pub arity: usize,
    pub func: fn(&[Value]) -> Result<Value, crate::error::ManoError>,
}

impl fmt::Debug for NativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeFunction")
            .field("name", &self.name)
            .field("arity", &self.arity)
            .finish()
    }
}

#[derive(Debug)]
pub struct ManoFunction {
    pub name: Option<Token>,
    pub params: Vec<Token>,
    pub body: Vec<Stmt>,
    pub closure: Rc<RefCell<Environment>>,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Literal(lit) => write!(f, "{}", lit),
            Value::Function(func) => write!(f, "{}", func),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Function::Mano(func) => match &func.name {
                Some(name) => write!(f, "<fita caseira {}>", name.lexeme),
                None => write!(f, "<fita caseira lambda>"),
            },
            Function::Native(func) => write!(f, "<fita raiz {}>", func.name),
        }
    }
}

impl From<Literal> for Value {
    fn from(lit: Literal) -> Self {
        Value::Literal(lit)
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Literal(a), Value::Literal(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::environment::Environment;
    use crate::token::{Token, TokenType};

    #[test]
    fn literal_value_displays_correctly() {
        let value = Value::Literal(Literal::Number(42.0));
        assert_eq!(value.to_string(), "42");
    }

    #[test]
    fn mano_function_displays_as_fita() {
        let func = ManoFunction {
            name: Some(Token {
                token_type: TokenType::Identifier,
                lexeme: "cumprimentar".to_string(),
                literal: None,
                span: 0..12,
            }),
            params: vec![],
            body: vec![],
            closure: Rc::new(RefCell::new(Environment::new())),
        };
        let value = Value::Function(Rc::new(Function::Mano(func)));
        assert_eq!(value.to_string(), "<fita caseira cumprimentar>");
    }

    #[test]
    fn lambda_displays_as_fita_lambda() {
        let func = ManoFunction {
            name: None,
            params: vec![],
            body: vec![],
            closure: Rc::new(RefCell::new(Environment::new())),
        };
        let value = Value::Function(Rc::new(Function::Mano(func)));
        assert_eq!(value.to_string(), "<fita caseira lambda>");
    }

    #[test]
    fn native_function_displays_as_nativo() {
        let func = NativeFunction {
            name: "fazTeuCorre".to_string(),
            arity: 0,
            func: |_| Ok(Value::Literal(Literal::Number(0.0))),
        };
        let value = Value::Function(Rc::new(Function::Native(func)));
        assert_eq!(value.to_string(), "<fita raiz fazTeuCorre>");
    }

    #[test]
    fn literal_values_are_equal() {
        let a = Value::Literal(Literal::Number(42.0));
        let b = Value::Literal(Literal::Number(42.0));
        assert_eq!(a, b);
    }

    #[test]
    fn same_function_rc_is_equal() {
        let func = Rc::new(Function::Mano(ManoFunction {
            name: Some(Token {
                token_type: TokenType::Identifier,
                lexeme: "test".to_string(),
                literal: None,
                span: 0..4,
            }),
            params: vec![],
            body: vec![],
            closure: Rc::new(RefCell::new(Environment::new())),
        }));
        let a = Value::Function(Rc::clone(&func));
        let b = Value::Function(Rc::clone(&func));
        assert_eq!(a, b);
    }

    #[test]
    fn literal_converts_to_value() {
        let lit = Literal::Number(42.0);
        let value: Value = lit.into();
        assert!(matches!(value, Value::Literal(Literal::Number(n)) if n == 42.0));
    }

    #[test]
    fn different_function_rcs_are_not_equal() {
        let make_func = || {
            Rc::new(Function::Mano(ManoFunction {
                name: Some(Token {
                    token_type: TokenType::Identifier,
                    lexeme: "test".to_string(),
                    literal: None,
                    span: 0..4,
                }),
                params: vec![],
                body: vec![],
                closure: Rc::new(RefCell::new(Environment::new())),
            }))
        };
        let a = Value::Function(make_func());
        let b = Value::Function(make_func());
        assert_ne!(a, b);
    }

    #[test]
    fn literal_and_function_are_not_equal() {
        let literal = Value::Literal(Literal::Number(42.0));
        let func = Value::Function(Rc::new(Function::Mano(ManoFunction {
            name: None,
            params: vec![],
            body: vec![],
            closure: Rc::new(RefCell::new(Environment::new())),
        })));
        assert_ne!(literal, func);
    }

    #[test]
    fn native_function_debug_shows_name_and_arity() {
        let func = NativeFunction {
            name: "fazTeuCorre".to_string(),
            arity: 0,
            func: |_| Ok(Value::Literal(Literal::Number(0.0))),
        };
        let debug_str = format!("{:?}", func);
        assert!(debug_str.contains("fazTeuCorre"));
        assert!(debug_str.contains("arity"));
    }
}
