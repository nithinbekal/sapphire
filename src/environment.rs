use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::value::{EnvRef, Value};

#[derive(Debug, Clone)]
pub struct Environment {
    values: HashMap<String, Value>,
    parent: Option<EnvRef>,
}

impl Environment {
    pub fn new() -> EnvRef {
        Rc::new(RefCell::new(Self { values: HashMap::new(), parent: None }))
    }

    pub fn new_child(parent: EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Self { values: HashMap::new(), parent: Some(parent) }))
    }

    pub fn set(&mut self, name: String, value: Value) {
        self.values.insert(name, value);
    }

    // Update an existing binding anywhere in the scope chain.
    // Returns true if found and updated, false if not found.
    pub fn assign(&mut self, name: &str, value: Value) -> bool {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            true
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().assign(name, value)
        } else {
            false
        }
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.values.get(name) {
            Some(v.clone())
        } else if let Some(parent) = &self.parent {
            parent.borrow().get(name)
        } else {
            None
        }
    }
}
