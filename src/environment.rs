use std::collections::HashMap;

pub struct Environment {
    values: HashMap<String, i64>,
}

impl Environment {
    pub fn new() -> Self {
        Self { values: HashMap::new() }
    }

    pub fn set(&mut self, name: String, value: i64) {
        self.values.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Option<i64> {
        self.values.get(name).copied()
    }
}
