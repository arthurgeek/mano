use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use crate::error::ManoError;
use crate::token::Value;

pub struct Environment {
    values: HashMap<String, Option<Value>>,
    enclosing: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            enclosing: None,
        }
    }

    pub fn with_enclosing(enclosing: Rc<RefCell<Environment>>) -> Self {
        Self {
            values: HashMap::new(),
            enclosing: Some(enclosing),
        }
    }

    pub fn define(&mut self, name: String, value: Value) {
        self.values.insert(name, Some(value));
    }

    pub fn define_uninitialized(&mut self, name: String) {
        self.values.insert(name, None);
    }

    pub fn get(&self, name: &str, span: Range<usize>) -> Result<Value, ManoError> {
        if let Some(maybe_value) = self.values.get(name) {
            return match maybe_value {
                Some(value) => Ok(value.clone()),
                None => Err(ManoError::Runtime {
                    message: format!(
                        "Variável '{}' tá vazia, chapa! Dá um valor pra ela primeiro!",
                        name
                    ),
                    span,
                }),
            };
        }

        if let Some(enclosing) = &self.enclosing {
            return enclosing.borrow().get(name, span);
        }

        Err(ManoError::Runtime {
            message: format!("Variável '{}' não existe, mano!", name),
            span,
        })
    }

    pub fn assign(
        &mut self,
        name: &str,
        value: Value,
        span: Range<usize>,
    ) -> Result<(), ManoError> {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), Some(value));
            return Ok(());
        }

        if let Some(enclosing) = &self.enclosing {
            return enclosing.borrow_mut().assign(name, value, span);
        }

        Err(ManoError::Runtime {
            message: format!("Variável '{}' não existe, mano!", name),
            span,
        })
    }

    pub fn variable_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.values.keys().cloned().collect();

        if let Some(enclosing) = &self.enclosing {
            for name in enclosing.borrow().variable_names() {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }

        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn define_and_get_variable() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Number(42.0));
        let result = env.get("x", 0..1).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn get_undefined_variable_returns_error() {
        let env = Environment::new();
        let result = env.get("x", 0..1);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn assign_updates_existing_variable() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Number(1.0));
        env.assign("x", Value::Number(42.0), 0..1).unwrap();
        let result = env.get("x", 0..1).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn assign_undefined_variable_returns_error() {
        let mut env = Environment::new();
        let result = env.assign("x", Value::Number(42.0), 0..1);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === enclosing scope tests ===

    #[test]
    fn get_from_enclosing_scope() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer
            .borrow_mut()
            .define("x".to_string(), Value::Number(42.0));

        let inner = Environment::with_enclosing(Rc::clone(&outer));
        let result = inner.get("x", 0..1).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn inner_shadows_outer() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer
            .borrow_mut()
            .define("x".to_string(), Value::Number(1.0));

        let mut inner = Environment::with_enclosing(Rc::clone(&outer));
        inner.define("x".to_string(), Value::Number(99.0));

        let result = inner.get("x", 0..1).unwrap();
        assert_eq!(result, Value::Number(99.0));
    }

    #[test]
    fn assign_updates_enclosing_scope() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer
            .borrow_mut()
            .define("x".to_string(), Value::Number(1.0));

        let mut inner = Environment::with_enclosing(Rc::clone(&outer));
        inner.assign("x", Value::Number(42.0), 0..1).unwrap();

        // Check outer was updated
        let result = outer.borrow().get("x", 0..1).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn get_uninitialized_variable_returns_error() {
        let mut env = Environment::new();
        env.define_uninitialized("x".to_string());
        let result = env.get("x", 0..1);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    #[test]
    fn assign_initializes_uninitialized_variable() {
        let mut env = Environment::new();
        env.define_uninitialized("x".to_string());
        env.assign("x", Value::Number(42.0), 0..1).unwrap();
        let result = env.get("x", 0..1).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn variable_names_returns_all_defined_names() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Number(1.0));
        env.define("nome".to_string(), Value::String("mano".to_string()));
        env.define_uninitialized("vazio".to_string());

        let names = env.variable_names();
        assert!(names.contains(&"x".to_string()));
        assert!(names.contains(&"nome".to_string()));
        assert!(names.contains(&"vazio".to_string()));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn variable_names_includes_enclosing_scopes() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer
            .borrow_mut()
            .define("outer_var".to_string(), Value::Number(1.0));

        let mut inner = Environment::with_enclosing(Rc::clone(&outer));
        inner.define("inner_var".to_string(), Value::Number(2.0));

        let names = inner.variable_names();
        assert!(names.contains(&"outer_var".to_string()));
        assert!(names.contains(&"inner_var".to_string()));
        assert_eq!(names.len(), 2);
    }
}
