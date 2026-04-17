use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Nil,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => {
                let s = format!("{}", n);
                if s.contains('.')
                    || s.contains('e')
                    || s.contains('E')
                    || s == "NaN"
                    || s.ends_with("inf")
                {
                    write!(f, "{}", s)
                } else {
                    write!(f, "{}.0", s)
                }
            }
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::Nil => write!(f, "nil"),
        }
    }
}
