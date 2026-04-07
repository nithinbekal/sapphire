use std::fmt;
use crate::value::Value;

#[derive(Debug)]
pub enum SapphireError {
    ParseError { message: String, line: usize },
    RuntimeError { message: String },
    TypeError { message: String },
    NonLocalReturn(Value, u64),
    Break(Value),
    Next(Value),
    Raised(Value),
}

impl fmt::Display for SapphireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SapphireError::ParseError { message, line } => {
                write!(f, "[line {}] parse error: {}", line, message)
            }
            SapphireError::RuntimeError { message } => {
                write!(f, "runtime error: {}", message)
            }
            SapphireError::TypeError { message } => {
                write!(f, "type error: {}", message)
            }
            SapphireError::NonLocalReturn(..) => write!(f, "return from block after method has returned"),
            SapphireError::Break(_)  => write!(f, "break outside of loop"),
            SapphireError::Next(_)   => write!(f, "next outside of loop"),
            SapphireError::Raised(v) => write!(f, "unhandled raise: {}", v),
        }
    }
}
