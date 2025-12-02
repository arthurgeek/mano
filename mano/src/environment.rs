use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use crate::error::ManoError;
use crate::value::Value;

#[derive(Debug, Default)]
pub struct Environment {
    values: HashMap<String, Option<Value>>,
    slots: Vec<Option<Value>>,
    slot_names: Vec<String>,
    enclosing: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_enclosing(enclosing: Rc<RefCell<Environment>>) -> Self {
        Self {
            values: HashMap::new(),
            slots: Vec::new(),
            slot_names: Vec::new(),
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

    /// Get variable by slot index (O(1) lookup)
    pub fn get_at(&self, distance: usize, slot: usize) -> Option<Value> {
        if distance == 0 {
            self.slots.get(slot).and_then(|v| v.clone())
        } else {
            self.enclosing
                .as_ref()
                .and_then(|enc| enc.borrow().get_at(distance - 1, slot))
        }
    }

    /// Assign variable by slot index (O(1) lookup)
    pub fn assign_at(&mut self, distance: usize, slot: usize, value: Value) {
        if distance == 0 {
            if slot < self.slots.len() {
                self.slots[slot] = Some(value);
            }
        } else if let Some(enclosing) = &self.enclosing {
            enclosing.borrow_mut().assign_at(distance - 1, slot, value);
        }
    }

    /// Define a value in the next slot
    pub fn define_at_slot(&mut self, name: String, value: Value) {
        self.slots.push(Some(value));
        self.slot_names.push(name);
    }

    /// Define an uninitialized slot (None)
    pub fn define_uninitialized_at_slot(&mut self, name: String) {
        self.slots.push(None);
        self.slot_names.push(name);
    }

    pub fn variable_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.values.keys().cloned().collect();

        // Add slot names (local variables)
        names.extend(self.slot_names.iter().cloned());

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
    use crate::token::Literal;

    fn num(n: f64) -> Value {
        Value::Literal(Literal::Number(n))
    }

    fn str(s: &str) -> Value {
        Value::Literal(Literal::String(s.to_string()))
    }

    #[test]
    fn define_and_get_variable() {
        let mut env = Environment::new();
        env.define("x".to_string(), num(42.0));
        let result = env.get("x", 0..1).unwrap();
        assert_eq!(result, num(42.0));
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
        env.define("x".to_string(), num(1.0));
        env.assign("x", num(42.0), 0..1).unwrap();
        let result = env.get("x", 0..1).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn assign_undefined_variable_returns_error() {
        let mut env = Environment::new();
        let result = env.assign("x", num(42.0), 0..1);
        assert!(matches!(result, Err(ManoError::Runtime { .. })));
    }

    // === enclosing scope tests ===

    #[test]
    fn get_from_enclosing_scope() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer.borrow_mut().define("x".to_string(), num(42.0));

        let inner = Environment::with_enclosing(Rc::clone(&outer));
        let result = inner.get("x", 0..1).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn inner_shadows_outer() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer.borrow_mut().define("x".to_string(), num(1.0));

        let mut inner = Environment::with_enclosing(Rc::clone(&outer));
        inner.define("x".to_string(), num(99.0));

        let result = inner.get("x", 0..1).unwrap();
        assert_eq!(result, num(99.0));
    }

    #[test]
    fn assign_updates_enclosing_scope() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer.borrow_mut().define("x".to_string(), num(1.0));

        let mut inner = Environment::with_enclosing(Rc::clone(&outer));
        inner.assign("x", num(42.0), 0..1).unwrap();

        // Check outer was updated
        let result = outer.borrow().get("x", 0..1).unwrap();
        assert_eq!(result, num(42.0));
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
        env.assign("x", num(42.0), 0..1).unwrap();
        let result = env.get("x", 0..1).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn variable_names_returns_all_defined_names() {
        let mut env = Environment::new();
        env.define("x".to_string(), num(1.0));
        env.define("nome".to_string(), str("mano"));
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
        outer.borrow_mut().define("outer_var".to_string(), num(1.0));

        let mut inner = Environment::with_enclosing(Rc::clone(&outer));
        inner.define("inner_var".to_string(), num(2.0));

        let names = inner.variable_names();
        assert!(names.contains(&"outer_var".to_string()));
        assert!(names.contains(&"inner_var".to_string()));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn variable_names_includes_slot_names() {
        let mut env = Environment::new();
        env.define_at_slot("x".to_string(), num(1.0));
        env.define_at_slot("y".to_string(), num(2.0));

        let names = env.variable_names();
        assert!(names.contains(&"x".to_string()));
        assert!(names.contains(&"y".to_string()));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn define_uninitialized_at_slot_creates_none_slot() {
        let mut env = Environment::new();
        env.define_uninitialized_at_slot("x".to_string());
        // Uninitialized slot returns None
        assert_eq!(env.get_at(0, 0), None);
        // But name is tracked
        let names = env.variable_names();
        assert!(names.contains(&"x".to_string()));
    }

    #[test]
    fn get_at_distance_0_returns_local() {
        let mut env = Environment::new();
        env.define_at_slot("x".to_string(), num(42.0)); // slot 0
        let result = env.get_at(0, 0).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn get_at_distance_1_returns_enclosing() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer
            .borrow_mut()
            .define_at_slot("x".to_string(), num(42.0)); // slot 0

        let inner = Environment::with_enclosing(Rc::clone(&outer));
        let result = inner.get_at(1, 0).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn assign_at_distance_0_updates_local() {
        let mut env = Environment::new();
        env.define_at_slot("x".to_string(), num(1.0)); // slot 0
        env.assign_at(0, 0, num(42.0));
        let result = env.get_at(0, 0).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn assign_at_distance_1_updates_enclosing() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer.borrow_mut().define_at_slot("x".to_string(), num(1.0)); // slot 0

        let mut inner = Environment::with_enclosing(Rc::clone(&outer));
        inner.define_at_slot("x".to_string(), num(0.0)); // slot 0 in inner
        inner.assign_at(1, 0, num(42.0));

        let result = outer.borrow().get_at(0, 0).unwrap();
        assert_eq!(result, num(42.0));
    }

    #[test]
    fn get_at_returns_value_by_index() {
        let mut env = Environment::new();
        env.define_at_slot("x".to_string(), num(42.0)); // slot 0
        env.define_at_slot("x".to_string(), num(99.0)); // slot 1

        assert_eq!(env.get_at(0, 0), Some(num(42.0)));
        assert_eq!(env.get_at(0, 1), Some(num(99.0)));
    }

    #[test]
    fn assign_at_updates_value_by_index() {
        let mut env = Environment::new();
        env.define_at_slot("x".to_string(), num(1.0)); // slot 0
        env.assign_at(0, 0, num(42.0));

        assert_eq!(env.get_at(0, 0), Some(num(42.0)));
    }
}
