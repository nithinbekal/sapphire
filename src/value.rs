use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use crate::ast::{Block, Expr, FieldDef, MethodDef, ParamDef, TypeExpr};
use crate::environment::Environment;

pub type EnvRef = Rc<RefCell<Environment>>;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Nil,
    Class {
        name: String,
        superclass: Option<String>,
        fields: Vec<FieldDef>,
        methods: Vec<MethodDef>,
        closure: EnvRef,
    },
    Constructor {
        class_name: String,
        fields: Vec<FieldDef>,
    },
    Instance {
        class_name: String,
        fields: Rc<RefCell<HashMap<String, Value>>>,
    },
    BoundMethod {
        receiver: Box<Value>,
        params: Vec<ParamDef>,
        return_type: Option<TypeExpr>,
        body: Vec<Expr>,
        closure: EnvRef,
        defined_in: String,
    },
    List(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<HashMap<String, Value>>>),
    NativeMethod {
        receiver: Box<Value>,
        name: String,
    },
    NativeFunction(String),
    Block(Block, EnvRef),
    Lambda {
        params: Vec<String>,
        body: Vec<Expr>,
        closure: EnvRef,
    },
    Range { from: i64, to: i64 },
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b)) => *a == (*b as f64),
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::Range { from: f1, to: t1 }, Value::Range { from: f2, to: t2 }) => f1 == f2 && t1 == t2,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => {
                let s = format!("{}", n);
                if s.contains('.') || s.contains('e') || s.contains('E') || s == "NaN" || s.ends_with("inf") {
                    write!(f, "{}", s)
                } else {
                    write!(f, "{}.0", s)
                }
            }
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::Nil => write!(f, "nil"),
            Value::Class { name, .. } => write!(f, "<class {}>", name),
            Value::Constructor { class_name, .. } => write!(f, "<constructor {}>", class_name),
            Value::BoundMethod { .. } => write!(f, "<method>"),
            Value::NativeMethod { name, .. } => write!(f, "<method {}>", name),
            Value::NativeFunction(name) => write!(f, "<fn {}>", name),
            Value::List(elements) => {
                let parts: Vec<String> = elements.borrow().iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Value::Map(pairs) => {
                let mut parts: Vec<String> = pairs.borrow().iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                parts.sort();
                write!(f, "{{{}}}", parts.join(", "))
            }
            Value::Block(..) => write!(f, "<block>"),
            Value::Lambda { .. } => write!(f, "<lambda>"),
            Value::Range { from, to } => write!(f, "{}..{}", from, to),
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
