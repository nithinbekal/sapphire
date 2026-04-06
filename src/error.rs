use std::fmt;
use crate::value::Value;

#[derive(Debug)]
pub enum SapphireError {
    ParseError { message: String, line: usize },
    RuntimeError { message: String },
    Return(Value),
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
            SapphireError::Return(_) => write!(f, "return outside of function"),
            SapphireError::Break(_)  => write!(f, "break outside of loop"),
            SapphireError::Next(_)   => write!(f, "next outside of loop"),
            SapphireError::Raised(v) => write!(f, "unhandled raise: {}", v),
        }
    }
}
