use std::fmt;

#[derive(Debug)]
pub enum SapphireError {
    ParseError { message: String, line: usize },
    RuntimeError { message: String },
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
        }
    }
}
