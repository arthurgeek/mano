use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::Stmt;
use crate::environment::Environment;
use crate::token::{Literal, Token};

#[derive(Debug, Clone)]
pub enum Value {
    Literal(Literal),
    Function(Rc<Function>),
    Class(Rc<Class>),
    Instance(Rc<Instance>),
}

#[derive(Debug)]
pub enum Function {
    Mano(ManoFunction),
    Native(NativeFunction),
}

impl Function {
    /// Bind this function to an instance (for method calls).
    /// Only ManoFunctions can be bound - native functions are never class methods.
    pub fn bind(&self, instance: Rc<Instance>) -> Function {
        let Function::Mano(f) = self else {
            unreachable!("Native functions are never class methods")
        };
        Function::Mano(f.bind(instance))
    }
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
    pub is_getter: bool,
}

impl ManoFunction {
    pub fn bind(&self, instance: Rc<Instance>) -> ManoFunction {
        let mut env = Environment::with_enclosing(Rc::clone(&self.closure));
        // Use slot-based storage for resolution to work
        env.define_at_slot("oCara".to_string(), Value::Instance(instance));
        ManoFunction {
            name: self.name.clone(),
            params: self.params.clone(),
            body: self.body.clone(),
            closure: Rc::new(RefCell::new(env)),
            is_getter: self.is_getter,
        }
    }
}

#[derive(Debug)]
pub struct Class {
    pub name: String,
    pub superclass: Option<Rc<Class>>,
    pub methods: HashMap<String, Rc<Function>>,
    pub static_methods: HashMap<String, Rc<Function>>,
}

impl Class {
    /// Find a method in this class or its superclass chain
    pub fn find_method(&self, name: &str) -> Option<Rc<Function>> {
        // First check this class
        if let Some(method) = self.methods.get(name) {
            return Some(Rc::clone(method));
        }

        // Then check superclass chain
        if let Some(ref superclass) = self.superclass {
            return superclass.find_method(name);
        }

        None
    }

    /// Find a static method in this class or its superclass chain
    pub fn find_static_method(&self, name: &str) -> Option<Rc<Function>> {
        // First check this class
        if let Some(method) = self.static_methods.get(name) {
            return Some(Rc::clone(method));
        }

        // Then check superclass chain
        if let Some(ref superclass) = self.superclass {
            return superclass.find_static_method(name);
        }

        None
    }
}

#[derive(Debug)]
pub struct Instance {
    pub class: Rc<Class>,
    pub fields: RefCell<HashMap<String, Value>>,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Literal(lit) => write!(f, "{}", lit),
            Value::Function(func) => write!(f, "{}", func),
            Value::Class(class) => write!(f, "<bagulho {}>", class.name),
            Value::Instance(instance) => write!(f, "<parada {}>", instance.class.name),
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
            (Value::Class(a), Value::Class(b)) => Rc::ptr_eq(a, b),
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
            is_getter: false,
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
            is_getter: false,
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
            is_getter: false,
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
                is_getter: false,
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
            is_getter: false,
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

    #[test]
    fn class_displays_as_bagulho() {
        let class = Class {
            name: "Pessoa".to_string(),
            superclass: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        };
        let value = Value::Class(Rc::new(class));
        assert_eq!(value.to_string(), "<bagulho Pessoa>");
    }

    #[test]
    fn same_class_rc_is_equal() {
        let class = Rc::new(Class {
            name: "Pessoa".to_string(),
            superclass: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        });
        let a = Value::Class(Rc::clone(&class));
        let b = Value::Class(Rc::clone(&class));
        assert_eq!(a, b);
    }

    #[test]
    fn different_class_rcs_are_not_equal() {
        let make_class = || {
            Rc::new(Class {
                name: "Pessoa".to_string(),
                superclass: None,
                methods: HashMap::new(),
                static_methods: HashMap::new(),
            })
        };
        let a = Value::Class(make_class());
        let b = Value::Class(make_class());
        assert_ne!(a, b);
    }

    #[test]
    fn class_with_superclass() {
        let parent = Rc::new(Class {
            name: "Pai".to_string(),
            superclass: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        });
        let child = Class {
            name: "Filho".to_string(),
            superclass: Some(Rc::clone(&parent)),
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        };
        assert!(child.superclass.is_some());
        assert_eq!(child.superclass.unwrap().name, "Pai");
    }

    #[test]
    fn instance_displays_as_parada() {
        let class = Rc::new(Class {
            name: "Pessoa".to_string(),
            superclass: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        });
        let instance = Instance {
            class: Rc::clone(&class),
            fields: RefCell::new(HashMap::new()),
        };
        let value = Value::Instance(Rc::new(instance));
        assert_eq!(value.to_string(), "<parada Pessoa>");
    }

    #[test]
    fn instance_fields_initially_empty() {
        let class = Rc::new(Class {
            name: "Pessoa".to_string(),
            superclass: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        });
        let instance = Instance {
            class: Rc::clone(&class),
            fields: RefCell::new(HashMap::new()),
        };
        assert!(instance.fields.borrow().is_empty());
    }

    #[test]
    fn instance_can_set_and_get_field() {
        let class = Rc::new(Class {
            name: "Pessoa".to_string(),
            superclass: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        });
        let instance = Instance {
            class: Rc::clone(&class),
            fields: RefCell::new(HashMap::new()),
        };
        instance.fields.borrow_mut().insert(
            "nome".to_string(),
            Value::Literal(Literal::String("João".to_string())),
        );
        let field = instance.fields.borrow().get("nome").cloned();
        assert!(matches!(
            field,
            Some(Value::Literal(Literal::String(ref s))) if s == "João"
        ));
    }

    #[test]
    fn bind_creates_closure_with_o_cara() {
        let class = Rc::new(Class {
            name: "Pessoa".to_string(),
            superclass: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
        });
        let instance = Rc::new(Instance {
            class: Rc::clone(&class),
            fields: RefCell::new(HashMap::new()),
        });

        let func = ManoFunction {
            name: Some(Token {
                token_type: TokenType::Identifier,
                lexeme: "falar".to_string(),
                literal: None,
                span: 0..5,
            }),
            params: vec![],
            body: vec![],
            closure: Rc::new(RefCell::new(Environment::new())),
            is_getter: false,
        };

        let bound = func.bind(Rc::clone(&instance));

        // The bound function should have oCara defined in its closure at slot 0
        let o_cara = bound.closure.borrow().get_at(0, 0).unwrap();
        assert!(matches!(o_cara, Value::Instance(_)));
    }
}
