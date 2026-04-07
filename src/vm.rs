use std::fmt;
use crate::chunk::{Chunk, Constant, OpCode};

// ── Runtime value ────────────────────────────────────────────────────────────

/// Values that live on the VM stack.
#[derive(Debug, Clone, PartialEq)]
pub enum VmValue {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Nil,
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmValue::Int(n)   => write!(f, "{}", n),
            VmValue::Float(n) => write!(f, "{}", n),
            VmValue::Str(s)   => write!(f, "{}", s),
            VmValue::Bool(b)  => write!(f, "{}", b),
            VmValue::Nil      => write!(f, "nil"),
        }
    }
}

impl From<&Constant> for VmValue {
    fn from(c: &Constant) -> Self {
        match c {
            Constant::Int(n)   => VmValue::Int(*n),
            Constant::Float(n) => VmValue::Float(*n),
            Constant::Str(s)   => VmValue::Str(s.clone()),
        }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum VmError {
    StackUnderflow,
    TypeError { message: String, line: u32 },
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::StackUnderflow => write!(f, "stack underflow"),
            VmError::TypeError { message, line } => {
                write!(f, "[line {}] type error: {}", line, message)
            }
        }
    }
}

// ── VM ───────────────────────────────────────────────────────────────────────

pub struct Vm<'a> {
    chunk: &'a Chunk,
    ip:    usize,
    stack: Vec<VmValue>,
}

impl<'a> Vm<'a> {
    pub fn new(chunk: &'a Chunk) -> Self {
        Vm { chunk, ip: 0, stack: Vec::new() }
    }

    /// Execute the chunk and return the top-of-stack value when `Return` is hit.
    pub fn run(&mut self) -> Result<Option<VmValue>, VmError> {
        loop {
            let op = &self.chunk.code[self.ip];
            let line = self.chunk.lines[self.ip];
            self.ip += 1;

            match op {
                OpCode::Constant(idx) => {
                    let val = VmValue::from(&self.chunk.constants[*idx]);
                    self.stack.push(val);
                }

                OpCode::True  => self.stack.push(VmValue::Bool(true)),
                OpCode::False => self.stack.push(VmValue::Bool(false)),
                OpCode::Nil   => self.stack.push(VmValue::Nil),

                OpCode::Jump(offset) => {
                    self.ip += offset;
                }

                OpCode::Loop(offset) => {
                    self.ip -= offset;
                }

                OpCode::JumpIfFalse(offset) => {
                    let cond = self.pop()?;
                    if is_falsy(&cond) {
                        self.ip += offset;
                    }
                }

                // `GetLocal` pushes a copy of the value at the given stack slot.
                OpCode::GetLocal(slot) => {
                    let val = self.stack.get(*slot)
                        .ok_or(VmError::StackUnderflow)?
                        .clone();
                    self.stack.push(val);
                }

                // `SetLocal` overwrites the slot but leaves the value on the stack
                // (assignment is an expression that returns the assigned value).
                OpCode::SetLocal(slot) => {
                    let val = self.stack.last()
                        .ok_or(VmError::StackUnderflow)?
                        .clone();
                    self.stack[*slot] = val;
                }

                OpCode::Pop => { self.pop()?; }

                OpCode::Negate => {
                    let v = self.pop()?;
                    self.stack.push(match v {
                        VmValue::Int(n)   => VmValue::Int(-n),
                        VmValue::Float(n) => VmValue::Float(-n),
                        other => return Err(VmError::TypeError {
                            message: format!("cannot negate {}", other),
                            line,
                        }),
                    });
                }

                OpCode::Not => {
                    let v = self.pop()?;
                    self.stack.push(VmValue::Bool(is_falsy(&v)));
                }

                OpCode::Add => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(match (&a, &b) {
                        (VmValue::Int(x),   VmValue::Int(y))   => VmValue::Int(x + y),
                        (VmValue::Float(x), VmValue::Float(y)) => VmValue::Float(x + y),
                        (VmValue::Int(x),   VmValue::Float(y)) => VmValue::Float(*x as f64 + y),
                        (VmValue::Float(x), VmValue::Int(y))   => VmValue::Float(x + *y as f64),
                        (VmValue::Str(x),   VmValue::Str(y))   => VmValue::Str(format!("{}{}", x, y)),
                        _ => return Err(VmError::TypeError {
                            message: format!("cannot add {} and {}", a, b),
                            line,
                        }),
                    });
                }

                OpCode::Sub => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(match (&a, &b) {
                        (VmValue::Int(x),   VmValue::Int(y))   => VmValue::Int(x - y),
                        (VmValue::Float(x), VmValue::Float(y)) => VmValue::Float(x - y),
                        (VmValue::Int(x),   VmValue::Float(y)) => VmValue::Float(*x as f64 - y),
                        (VmValue::Float(x), VmValue::Int(y))   => VmValue::Float(x - *y as f64),
                        _ => return Err(VmError::TypeError {
                            message: format!("cannot subtract {} and {}", a, b),
                            line,
                        }),
                    });
                }

                OpCode::Mul => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(match (&a, &b) {
                        (VmValue::Int(x),   VmValue::Int(y))   => VmValue::Int(x * y),
                        (VmValue::Float(x), VmValue::Float(y)) => VmValue::Float(x * y),
                        (VmValue::Int(x),   VmValue::Float(y)) => VmValue::Float(*x as f64 * y),
                        (VmValue::Float(x), VmValue::Int(y))   => VmValue::Float(x * *y as f64),
                        _ => return Err(VmError::TypeError {
                            message: format!("cannot multiply {} and {}", a, b),
                            line,
                        }),
                    });
                }

                OpCode::Div => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(match (&a, &b) {
                        (VmValue::Int(x),   VmValue::Int(y))   => VmValue::Int(x / y),
                        (VmValue::Float(x), VmValue::Float(y)) => VmValue::Float(x / y),
                        (VmValue::Int(x),   VmValue::Float(y)) => VmValue::Float(*x as f64 / y),
                        (VmValue::Float(x), VmValue::Int(y))   => VmValue::Float(x / *y as f64),
                        _ => return Err(VmError::TypeError {
                            message: format!("cannot divide {} and {}", a, b),
                            line,
                        }),
                    });
                }

                OpCode::Equal        => { let (a, b) = self.pop2()?; self.stack.push(VmValue::Bool(a == b)); }
                OpCode::NotEqual     => { let (a, b) = self.pop2()?; self.stack.push(VmValue::Bool(a != b)); }

                OpCode::Less => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(VmValue::Bool(numeric_cmp(&a, &b, line, |x, y| x < y)?));
                }
                OpCode::LessEqual => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(VmValue::Bool(numeric_cmp(&a, &b, line, |x, y| x <= y)?));
                }
                OpCode::Greater => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(VmValue::Bool(numeric_cmp(&a, &b, line, |x, y| x > y)?));
                }
                OpCode::GreaterEqual => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(VmValue::Bool(numeric_cmp(&a, &b, line, |x, y| x >= y)?));
                }

                OpCode::Return => {
                    return Ok(self.stack.pop());
                }
            }
        }
    }

    fn pop(&mut self) -> Result<VmValue, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    /// Pop two values; the first pushed is `a`, last pushed is `b`.
    fn pop2(&mut self) -> Result<(VmValue, VmValue), VmError> {
        let b = self.pop()?;
        let a = self.pop()?;
        Ok((a, b))
    }
}

fn is_falsy(v: &VmValue) -> bool {
    matches!(v, VmValue::Nil | VmValue::Bool(false))
}

fn numeric_cmp(
    a: &VmValue,
    b: &VmValue,
    line: u32,
    op: impl Fn(f64, f64) -> bool,
) -> Result<bool, VmError> {
    let x = to_float(a).ok_or_else(|| VmError::TypeError {
        message: format!("cannot compare {} and {}", a, b),
        line,
    })?;
    let y = to_float(b).ok_or_else(|| VmError::TypeError {
        message: format!("cannot compare {} and {}", a, b),
        line,
    })?;
    Ok(op(x, y))
}

fn to_float(v: &VmValue) -> Option<f64> {
    match v {
        VmValue::Int(n)   => Some(*n as f64),
        VmValue::Float(n) => Some(*n),
        _                 => None,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{Chunk, Constant, OpCode};

    fn run(chunk: Chunk) -> Result<Option<VmValue>, VmError> {
        Vm::new(&chunk).run()
    }

    fn chunk_with(ops: impl Fn(&mut Chunk)) -> Chunk {
        let mut c = Chunk::new();
        ops(&mut c);
        c
    }

    #[test]
    fn returns_constant() {
        let chunk = chunk_with(|c| {
            let i = c.add_constant(Constant::Int(42));
            c.write(OpCode::Constant(i), 1);
            c.write(OpCode::Return, 1);
        });
        assert_eq!(run(chunk).unwrap(), Some(VmValue::Int(42)));
    }

    #[test]
    fn addition_int() {
        let chunk = chunk_with(|c| {
            let a = c.add_constant(Constant::Int(10));
            let b = c.add_constant(Constant::Int(32));
            c.write(OpCode::Constant(a), 1);
            c.write(OpCode::Constant(b), 1);
            c.write(OpCode::Add, 1);
            c.write(OpCode::Return, 1);
        });
        assert_eq!(run(chunk).unwrap(), Some(VmValue::Int(42)));
    }

    #[test]
    fn addition_mixed() {
        let chunk = chunk_with(|c| {
            let a = c.add_constant(Constant::Int(1));
            let b = c.add_constant(Constant::Float(1.5));
            c.write(OpCode::Constant(a), 1);
            c.write(OpCode::Constant(b), 1);
            c.write(OpCode::Add, 1);
            c.write(OpCode::Return, 1);
        });
        assert_eq!(run(chunk).unwrap(), Some(VmValue::Float(2.5)));
    }

    #[test]
    fn negation() {
        let chunk = chunk_with(|c| {
            let i = c.add_constant(Constant::Int(7));
            c.write(OpCode::Constant(i), 1);
            c.write(OpCode::Negate, 1);
            c.write(OpCode::Return, 1);
        });
        assert_eq!(run(chunk).unwrap(), Some(VmValue::Int(-7)));
    }

    #[test]
    fn not_false_is_true() {
        let chunk = chunk_with(|c| {
            c.write(OpCode::False, 1);
            c.write(OpCode::Not, 1);
            c.write(OpCode::Return, 1);
        });
        assert_eq!(run(chunk).unwrap(), Some(VmValue::Bool(true)));
    }

    #[test]
    fn comparison_less() {
        let chunk = chunk_with(|c| {
            let a = c.add_constant(Constant::Int(3));
            let b = c.add_constant(Constant::Int(5));
            c.write(OpCode::Constant(a), 1);
            c.write(OpCode::Constant(b), 1);
            c.write(OpCode::Less, 1);
            c.write(OpCode::Return, 1);
        });
        assert_eq!(run(chunk).unwrap(), Some(VmValue::Bool(true)));
    }

    #[test]
    fn string_concat() {
        let chunk = chunk_with(|c| {
            let a = c.add_constant(Constant::Str("hello".into()));
            let b = c.add_constant(Constant::Str(" world".into()));
            c.write(OpCode::Constant(a), 1);
            c.write(OpCode::Constant(b), 1);
            c.write(OpCode::Add, 1);
            c.write(OpCode::Return, 1);
        });
        assert_eq!(run(chunk).unwrap(), Some(VmValue::Str("hello world".into())));
    }

    #[test]
    fn type_error_on_bad_add() {
        let chunk = chunk_with(|c| {
            let a = c.add_constant(Constant::Int(1));
            c.write(OpCode::Constant(a), 1);
            c.write(OpCode::True, 1);
            c.write(OpCode::Add, 1);
            c.write(OpCode::Return, 1);
        });
        assert!(matches!(run(chunk), Err(VmError::TypeError { .. })));
    }
}
