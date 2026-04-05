use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use crate::ast::{FieldDef, MethodDef, Stmt};
use crate::environment::Environment;

pub type EnvRef = Rc<RefCell<Environment>>;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Nil,
    Function {
        params: Vec<String>,
        body: Vec<Stmt>,
        closure: EnvRef,
    },
    Class {
        name: String,
        fields: Vec<FieldDef>,
        methods: Vec<MethodDef>,
        closure: EnvRef,
    },
    Constructor {
        class_name: String,
        fields: Vec<FieldDef>,
        methods: Vec<MethodDef>,
        closure: EnvRef,
    },
    Instance {
        class_name: String,
        fields: Rc<RefCell<HashMap<String, Value>>>,
    },
    BoundMethod {
        receiver: Box<Value>,
        params: Vec<String>,
        body: Vec<Stmt>,
        closure: EnvRef,
    },
    List(Rc<RefCell<Vec<Value>>>),
    NativeMethod {
        receiver: Box<Value>,
        name: String,
    },
    NativeFunction(String),
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
            Value::BoundMethod { .. } => write!(f, "<method>"),
            Value::NativeMethod { name, .. } => write!(f, "<method {}>", name),
            Value::NativeFunction(name) => write!(f, "<fn {}>", name),
            Value::List(elements) => {
                let parts: Vec<String> = elements.borrow().iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Value::Instance { class_name, fields } => {
                let mut pairs: Vec<String> = fields
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                pairs.sort();
                write!(f, "#<{} {}>", class_name, pairs.join(", "))
            }
        }
    }
}
