use std::fmt;

#[derive(Debug)]
pub enum SapphireError {
    ParseError { message: String, line: usize, column: usize },
    RuntimeError { message: String },
    TypeError { message: String, line: usize },
}

impl fmt::Display for SapphireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SapphireError::ParseError { message, line, column } => {
                write!(f, "[line {}:{}] parse error: {}", line, column, message)
            }
            SapphireError::RuntimeError { message } => {
                write!(f, "runtime error: {}", message)
            }
            SapphireError::TypeError { message, line } if *line > 0 => {
                write!(f, "[line {}] type error: {}", line, message)
            }
            SapphireError::TypeError { message, .. } => {
                write!(f, "type error: {}", message)
            }
        }
    }
}
