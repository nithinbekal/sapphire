use std::fmt;
use std::rc::Rc;
use crate::chunk::{Constant, Function, OpCode};

// ── Runtime value ────────────────────────────────────────────────────────────

/// Values that live on the VM stack.
#[derive(Debug, Clone, PartialEq)]
pub enum VmValue {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Nil,
    Function(Rc<Function>),
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmValue::Int(n)      => write!(f, "{}", n),
            VmValue::Float(n)    => write!(f, "{}", n),
            VmValue::Str(s)      => write!(f, "{}", s),
            VmValue::Bool(b)     => write!(f, "{}", b),
            VmValue::Nil         => write!(f, "nil"),
            VmValue::Function(func) => write!(f, "<fn {}>", func.name),
        }
    }
}

impl From<&Constant> for VmValue {
    fn from(c: &Constant) -> Self {
        match c {
            Constant::Int(n)         => VmValue::Int(*n),
            Constant::Float(n)       => VmValue::Float(*n),
            Constant::Str(s)         => VmValue::Str(s.clone()),
            Constant::Function(func) => VmValue::Function(func.clone()),
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

// ── Call frame ───────────────────────────────────────────────────────────────

struct CallFrame {
    function: Rc<Function>,
    /// Instruction pointer within this frame's chunk.
    ip:   usize,
    /// Index into the VM stack where slot 0 of this frame begins.
    /// The function value itself lives at `base - 1`.
    base: usize,
}

// ── VM ───────────────────────────────────────────────────────────────────────

pub struct Vm {
    frames: Vec<CallFrame>,
    stack:  Vec<VmValue>,
}

impl Vm {
    pub fn new(function: Rc<Function>) -> Self {
        let frame = CallFrame { function, ip: 0, base: 0 };
        Vm { frames: vec![frame], stack: Vec::new() }
    }

    pub fn run(&mut self) -> Result<Option<VmValue>, VmError> {
        loop {
            // Fetch the next instruction from the current frame, then release the borrow.
            let (op, line) = {
                let frame = self.frames.last_mut().unwrap();
                let op   = frame.function.chunk.code[frame.ip].clone();
                let line = frame.function.chunk.lines[frame.ip];
                frame.ip += 1;
                (op, line)
            };

            match op {
                OpCode::Constant(idx) => {
                    let val = VmValue::from(
                        &self.frames.last().unwrap().function.chunk.constants[idx]
                    );
                    self.stack.push(val);
                }

                OpCode::True  => self.stack.push(VmValue::Bool(true)),
                OpCode::False => self.stack.push(VmValue::Bool(false)),
                OpCode::Nil   => self.stack.push(VmValue::Nil),

                OpCode::Pop => { self.pop()?; }

                // Local slots are relative to the current frame's base.
                OpCode::GetLocal(slot) => {
                    let base = self.frames.last().unwrap().base;
                    let val  = self.stack[base + slot].clone();
                    self.stack.push(val);
                }
                OpCode::SetLocal(slot) => {
                    let base = self.frames.last().unwrap().base;
                    let val  = self.stack.last().ok_or(VmError::StackUnderflow)?.clone();
                    self.stack[base + slot] = val;
                }

                OpCode::Jump(offset) => {
                    self.frames.last_mut().unwrap().ip += offset;
                }
                OpCode::JumpIfFalse(offset) => {
                    let cond = self.pop()?;
                    if is_falsy(&cond) {
                        self.frames.last_mut().unwrap().ip += offset;
                    }
                }
                OpCode::Loop(offset) => {
                    self.frames.last_mut().unwrap().ip -= offset;
                }

                OpCode::Call(arg_count) => {
                    // Stack: [..., fn, arg0, arg1, ..., argN-1]
                    let fn_slot = self.stack.len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;

                    let function = match &self.stack[fn_slot] {
                        VmValue::Function(f) => f.clone(),
                        other => return Err(VmError::TypeError {
                            message: format!("'{}' is not callable", other),
                            line,
                        }),
                    };
                    if function.arity != arg_count {
                        return Err(VmError::TypeError {
                            message: format!(
                                "'{}' expects {} argument(s), got {}",
                                function.name, function.arity, arg_count
                            ),
                            line,
                        });
                    }
                    // Slot 0 of the new frame is the function itself (enables recursion).
                    // Slot 1..arity are the arguments.
                    let base = fn_slot;
                    self.frames.push(CallFrame { function, ip: 0, base });
                }

                OpCode::Return => {
                    let return_val = self.stack.pop();
                    let frame = self.frames.pop().unwrap();

                    if self.frames.is_empty() {
                        return Ok(return_val);
                    }

                    // Discard the function value and all frame locals.
                    self.stack.truncate(frame.base);
                    self.stack.push(return_val.unwrap_or(VmValue::Nil));
                }

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
            }
        }
    }

    fn pop(&mut self) -> Result<VmValue, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

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
    use crate::chunk::{Chunk, Constant, Function, OpCode};

    fn run(chunk: Chunk) -> Result<Option<VmValue>, VmError> {
        let f = Rc::new(Function { name: String::new(), arity: 0, chunk });
        Vm::new(f).run()
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
