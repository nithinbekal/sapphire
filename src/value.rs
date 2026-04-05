use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use crate::ast::{FieldDef, Stmt};
use crate::environment::Environment;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Nil,
    Function {
        params: Vec<String>,
        body: Vec<Stmt>,
        closure: Rc<RefCell<Environment>>,
    },
    Class {
        name: String,
        fields: Vec<FieldDef>,
    },
    Constructor {
        class_name: String,
        fields: Vec<FieldDef>,
    },
    Instance {
        class_name: String,
        fields: HashMap<String, Value>,
    },
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::Nil => write!(f, "nil"),
            Value::Function { params, .. } => write!(f, "<fn({})>", params.join(", ")),
            Value::Class { name, .. } => write!(f, "<class {}>", name),
            Value::Constructor { class_name, .. } => write!(f, "<new {}>", class_name),
            Value::Instance { class_name, fields } => {
                let pairs: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                write!(f, "#<{} {}>", class_name, pairs.join(", "))
            }
        }
    }
}
