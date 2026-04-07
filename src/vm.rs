use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use crate::chunk::{Constant, Function, OpCode};

// ── Upvalue ───────────────────────────────────────────────────────────────────

/// The heap-allocated cell shared between a closure and the variable it captures.
/// While the captured variable is still live on the stack the upvalue is "open"
/// (holds a stack index).  When the enclosing frame returns the upvalue is
/// "closed": the value is copied out of the stack into the cell itself.
#[derive(Debug, Clone)]
pub enum UpvalueState {
    Open(usize),       // index into Vm::stack
    Closed(VmValue),
}

#[derive(Debug, Clone)]
pub struct Upvalue(pub Rc<RefCell<UpvalueState>>);

impl Upvalue {
    fn new_open(stack_idx: usize) -> Self {
        Upvalue(Rc::new(RefCell::new(UpvalueState::Open(stack_idx))))
    }
}

impl PartialEq for Upvalue {
    fn eq(&self, other: &Self) -> bool { Rc::ptr_eq(&self.0, &other.0) }
}

// ── Runtime value ─────────────────────────────────────────────────────────────

/// Values that live on the VM stack.
#[derive(Debug, Clone)]
pub enum VmValue {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Nil,
    /// A bare function value (no upvalues).  Used only when a Function constant
    /// is loaded directly via `OpCode::Constant`; the compiler always uses
    /// `OpCode::Closure` instead.
    Function(Rc<Function>),
    /// A closure: a function paired with its captured upvalues.
    Closure {
        function: Rc<Function>,
        upvalues: Vec<Upvalue>,
    },
    List(Rc<RefCell<Vec<VmValue>>>),
    Map(Rc<RefCell<HashMap<String, VmValue>>>),
    Range { from: i64, to: i64 },
    /// A compiled class: holds the static field list and the method table.
    Class {
        name:    String,
        fields:  Vec<String>,
        methods: Rc<HashMap<String, VmMethod>>,
    },
    /// A live instance of a class.
    Instance {
        class_name: String,
        fields:     Rc<RefCell<HashMap<String, VmValue>>>,
        methods:    Rc<HashMap<String, VmMethod>>,
    },
}

/// A compiled method: a function together with any upvalues it closed over.
#[derive(Debug, Clone)]
pub struct VmMethod {
    pub function: Rc<Function>,
    pub upvalues: Vec<Upvalue>,
}

impl PartialEq for VmValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VmValue::Int(a),   VmValue::Int(b))   => a == b,
            (VmValue::Float(a), VmValue::Float(b)) => a == b,
            (VmValue::Str(a),   VmValue::Str(b))   => a == b,
            (VmValue::Bool(a),  VmValue::Bool(b))  => a == b,
            (VmValue::Nil,      VmValue::Nil)      => true,
            (VmValue::Function(a), VmValue::Function(b)) => Rc::ptr_eq(a, b),
            (VmValue::Closure { function: f1, .. },
             VmValue::Closure { function: f2, .. }) => Rc::ptr_eq(f1, f2),
            (VmValue::List(a),  VmValue::List(b))  => Rc::ptr_eq(a, b),
            (VmValue::Map(a),   VmValue::Map(b))   => Rc::ptr_eq(a, b),
            (VmValue::Range { from: f1, to: t1 },
             VmValue::Range { from: f2, to: t2 })  => f1 == f2 && t1 == t2,
            (VmValue::Instance { fields: f1, .. },
             VmValue::Instance { fields: f2, .. }) => Rc::ptr_eq(f1, f2),
            _ => false,
        }
    }
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmValue::Int(n)      => write!(f, "{}", n),
            VmValue::Float(n)    => write!(f, "{}", n),
            VmValue::Str(s)      => write!(f, "{}", s),
            VmValue::Bool(b)     => write!(f, "{}", b),
            VmValue::Nil         => write!(f, "nil"),
            VmValue::Function(func)          => write!(f, "<fn {}>", func.name),
            VmValue::Closure { function, .. } => write!(f, "<fn {}>", function.name),
            VmValue::List(elems) => {
                let parts: Vec<String> = elems.borrow().iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            VmValue::Map(pairs) => {
                let mut parts: Vec<String> = pairs.borrow().iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                parts.sort();
                write!(f, "{{{}}}", parts.join(", "))
            }
            VmValue::Range { from, to } => write!(f, "{}..{}", from, to),
            VmValue::Class { name, .. }  => write!(f, "<class {}>", name),
            VmValue::Instance { class_name, fields, .. } => {
                let mut pairs: Vec<String> = fields.borrow().iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                pairs.sort();
                write!(f, "#<{} {}>", class_name, pairs.join(", "))
            }
        }
    }
}

impl From<&Constant> for VmValue {
    fn from(c: &Constant) -> Self {
        match c {
            Constant::Int(n)         => VmValue::Int(*n),
            Constant::Float(n)       => VmValue::Float(*n),
            Constant::Str(s)         => VmValue::Str(s.clone()),
            Constant::Function(func)    => VmValue::Function(func.clone()),
            Constant::ClassDesc { .. }  => panic!("ClassDesc cannot be used as a stack value"),
        }
    }
}

// ── Error ─────────────────────────────────────────────────────────────────────

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

// ── Call frame ────────────────────────────────────────────────────────────────

struct CallFrame {
    function: Rc<Function>,
    /// The upvalues belonging to the closure that created this frame.
    upvalues: Vec<Upvalue>,
    /// Instruction pointer within this frame's chunk.
    ip:       usize,
    /// Index into the VM stack where slot 0 of this frame begins.
    /// The function value itself lives at `base`.
    base:     usize,
    /// Block passed to this call (if any), stored off-stack so `yield` can
    /// reach it without disturbing the locals layout.
    block: Option<VmMethod>,
}

// ── VM ────────────────────────────────────────────────────────────────────────

pub struct Vm {
    frames:        Vec<CallFrame>,
    stack:         Vec<VmValue>,
    /// All upvalues that still point into the live stack (open upvalues),
    /// kept so we can close them when a frame exits.
    open_upvalues: Vec<Upvalue>,
}

impl Vm {
    pub fn new(function: Rc<Function>) -> Self {
        let frame = CallFrame { function, upvalues: vec![], ip: 0, base: 0, block: None };
        Vm { frames: vec![frame], stack: Vec::new(), open_upvalues: Vec::new() }
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

                OpCode::Closure(idx) => {
                    // Load the function from the current frame's constant pool.
                    let func = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Function(f) => f.clone(),
                        _ => panic!("Closure opcode on non-function constant"),
                    };
                    // Collect upvalue descriptors into a plain vec so we can call
                    // &mut self methods without holding a borrow on the frame.
                    let defs: Vec<(bool, usize)> = func.upvalue_defs
                        .iter()
                        .map(|d| (d.is_local, d.index))
                        .collect();
                    let mut upvalues = Vec::with_capacity(defs.len());
                    for (is_local, index) in defs {
                        let uv = if is_local {
                            let base = self.frames.last().unwrap().base;
                            self.capture_upvalue(base + index)
                        } else {
                            self.frames.last().unwrap().upvalues[index].clone()
                        };
                        upvalues.push(uv);
                    }
                    self.stack.push(VmValue::Closure { function: func, upvalues });
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

                OpCode::GetUpvalue(idx) => {
                    let uv = self.frames.last().unwrap().upvalues[idx].clone();
                    let val = match &*uv.0.borrow() {
                        UpvalueState::Open(stack_idx) => self.stack[*stack_idx].clone(),
                        UpvalueState::Closed(val)     => val.clone(),
                    };
                    self.stack.push(val);
                }
                OpCode::SetUpvalue(idx) => {
                    let uv  = self.frames.last().unwrap().upvalues[idx].clone();
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?.clone();
                    match &mut *uv.0.borrow_mut() {
                        UpvalueState::Open(stack_idx) => { self.stack[*stack_idx] = val; }
                        UpvalueState::Closed(v)       => { *v = val; }
                    }
                }
                OpCode::CloseUpvalue => {
                    let top = self.stack.len() - 1;
                    self.close_upvalues_above(top);
                    self.stack.pop();
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

                // Short-circuit: peek TOS; if falsy, keep it and jump; else pop and fall through.
                OpCode::JumpIfFalseKeep(offset) => {
                    let falsy = is_falsy(self.stack.last().ok_or(VmError::StackUnderflow)?);
                    if falsy {
                        self.frames.last_mut().unwrap().ip += offset;
                    } else {
                        self.stack.pop();
                    }
                }
                // Short-circuit: peek TOS; if truthy, keep it and jump; else pop and fall through.
                OpCode::JumpIfTrueKeep(offset) => {
                    let truthy = !is_falsy(self.stack.last().ok_or(VmError::StackUnderflow)?);
                    if truthy {
                        self.frames.last_mut().unwrap().ip += offset;
                    } else {
                        self.stack.pop();
                    }
                }

                OpCode::BuildString(n) => {
                    let start = self.stack.len().checked_sub(n).ok_or(VmError::StackUnderflow)?;
                    let parts: Vec<String> = self.stack.drain(start..)
                        .map(|v| format!("{}", v))
                        .collect();
                    self.stack.push(VmValue::Str(parts.concat()));
                }

                OpCode::BuildList(n) => {
                    let start = self.stack.len().checked_sub(n).ok_or(VmError::StackUnderflow)?;
                    let elems: Vec<VmValue> = self.stack.drain(start..).collect();
                    self.stack.push(VmValue::List(Rc::new(RefCell::new(elems))));
                }

                OpCode::BuildMap(n) => {
                    // Stack layout: key0, val0, key1, val1, ... (2*n values)
                    let start = self.stack.len().checked_sub(n * 2).ok_or(VmError::StackUnderflow)?;
                    let flat: Vec<VmValue> = self.stack.drain(start..).collect();
                    let mut map = HashMap::new();
                    for chunk in flat.chunks(2) {
                        let key = match &chunk[0] {
                            VmValue::Str(s) => s.clone(),
                            other => return Err(VmError::TypeError {
                                message: format!("map key must be a string, got {}", other),
                                line,
                            }),
                        };
                        map.insert(key, chunk[1].clone());
                    }
                    self.stack.push(VmValue::Map(Rc::new(RefCell::new(map))));
                }

                OpCode::BuildRange => {
                    let (a, b) = self.pop2()?;
                    match (&a, &b) {
                        (VmValue::Int(from), VmValue::Int(to)) => {
                            self.stack.push(VmValue::Range { from: *from, to: *to });
                        }
                        _ => return Err(VmError::TypeError {
                            message: format!("range bounds must be integers, got {} and {}", a, b),
                            line,
                        }),
                    }
                }

                OpCode::Index => {
                    let idx = self.pop()?;
                    let obj = self.pop()?;
                    match (obj, &idx) {
                        (VmValue::List(elems), VmValue::Int(i)) => {
                            let elems = elems.borrow();
                            let len = elems.len() as i64;
                            let i = if *i < 0 { len + i } else { *i };
                            if i < 0 || i >= len {
                                return Err(VmError::TypeError {
                                    message: format!("list index {} out of bounds (len {})", idx, len),
                                    line,
                                });
                            }
                            self.stack.push(elems[i as usize].clone());
                        }
                        (VmValue::Map(map), VmValue::Str(key)) => {
                            let val = map.borrow().get(key).cloned().unwrap_or(VmValue::Nil);
                            self.stack.push(val);
                        }
                        (VmValue::Str(s), VmValue::Int(i)) => {
                            let chars: Vec<char> = s.chars().collect();
                            let len = chars.len() as i64;
                            let i = if *i < 0 { len + i } else { *i };
                            if i < 0 || i >= len {
                                return Err(VmError::TypeError {
                                    message: format!("string index {} out of bounds", idx),
                                    line,
                                });
                            }
                            self.stack.push(VmValue::Str(chars[i as usize].to_string()));
                        }
                        (obj, _) => return Err(VmError::TypeError {
                            message: format!("cannot index {} with {}", obj, idx),
                            line,
                        }),
                    }
                }

                OpCode::IndexSet => {
                    let val = self.pop()?;
                    let idx = self.pop()?;
                    let obj = self.pop()?;
                    match (obj, idx) {
                        (VmValue::List(elems), VmValue::Int(i)) => {
                            let mut elems = elems.borrow_mut();
                            let len = elems.len() as i64;
                            let i = if i < 0 { len + i } else { i };
                            if i < 0 || i >= len {
                                return Err(VmError::TypeError {
                                    message: format!("list index {} out of bounds", i),
                                    line,
                                });
                            }
                            elems[i as usize] = val.clone();
                        }
                        (VmValue::Map(map), VmValue::Str(key)) => {
                            map.borrow_mut().insert(key, val.clone());
                        }
                        (obj, idx) => return Err(VmError::TypeError {
                            message: format!("cannot index-assign {} with {}", obj, idx),
                            line,
                        }),
                    }
                    self.stack.push(val);
                }

                // ── Class opcodes ────────────────────────────────────────────

                OpCode::DefClass(desc_idx) => {
                    let (class_name, field_names, method_names) = {
                        let consts = &self.frames.last().unwrap().function.chunk.constants;
                        match &consts[desc_idx] {
                            Constant::ClassDesc { name, field_names, method_names } =>
                                (name.clone(), field_names.clone(), method_names.clone()),
                            _ => panic!("DefClass: expected ClassDesc constant"),
                        }
                    };
                    let n = method_names.len();
                    let start = self.stack.len().checked_sub(n).ok_or(VmError::StackUnderflow)?;
                    let closures: Vec<VmValue> = self.stack.drain(start..).collect();
                    let mut methods = HashMap::new();
                    for (name, closure) in method_names.iter().zip(closures) {
                        match closure {
                            VmValue::Closure { function, upvalues } =>
                                { methods.insert(name.clone(), VmMethod { function, upvalues }); }
                            _ => panic!("DefClass: method is not a closure"),
                        }
                    }
                    self.stack.push(VmValue::Class {
                        name:    class_name,
                        fields:  field_names,
                        methods: Rc::new(methods),
                    });
                }

                OpCode::NewInstance(n_pairs) => {
                    // Stack: [class, name0, val0, …, nameN, valN]
                    let base = self.stack.len()
                        .checked_sub(1 + n_pairs * 2)
                        .ok_or(VmError::StackUnderflow)?;
                    let (class_name, field_decls, methods) = match &self.stack[base] {
                        VmValue::Class { name, fields, methods } =>
                            (name.clone(), fields.clone(), methods.clone()),
                        other => return Err(VmError::TypeError {
                            message: format!("'{}' is not a class", other),
                            line,
                        }),
                    };
                    // Initialise all declared fields to nil.
                    let mut instance_fields: HashMap<String, VmValue> =
                        field_decls.iter().map(|n| (n.clone(), VmValue::Nil)).collect();
                    // Apply named constructor arguments.
                    for i in 0..n_pairs {
                        let name_val = self.stack[base + 1 + i * 2].clone();
                        let val      = self.stack[base + 2 + i * 2].clone();
                        match name_val {
                            VmValue::Str(ref n) if !n.is_empty() => {
                                instance_fields.insert(n.clone(), val);
                            }
                            _ => {}
                        }
                    }
                    self.stack.drain(base..);
                    self.stack.push(VmValue::Instance {
                        class_name,
                        fields:  Rc::new(RefCell::new(instance_fields)),
                        methods,
                    });
                }

                OpCode::GetField(idx) => {
                    let name = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Str(s) => s.clone(),
                        _ => panic!("GetField: expected Str constant"),
                    };
                    let obj = self.pop()?;
                    match obj {
                        VmValue::Instance { ref fields, .. } => {
                            let val = fields.borrow().get(&name).cloned().unwrap_or(VmValue::Nil);
                            self.stack.push(val);
                        }
                        other => return Err(VmError::TypeError {
                            message: format!("cannot get field '{}' on {}", name, other),
                            line,
                        }),
                    }
                }

                OpCode::SetField(idx) => {
                    let name = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Str(s) => s.clone(),
                        _ => panic!("SetField: expected Str constant"),
                    };
                    let val = self.pop()?;
                    let obj = self.pop()?;
                    match obj {
                        VmValue::Instance { ref fields, .. } => {
                            fields.borrow_mut().insert(name, val.clone());
                            self.stack.push(val);
                        }
                        other => return Err(VmError::TypeError {
                            message: format!("cannot set field '{}' on {}", name, other),
                            line,
                        }),
                    }
                }

                OpCode::Invoke(name_idx, arg_count) => {
                    let method_name = match &self.frames.last().unwrap().function.chunk.constants[name_idx] {
                        Constant::Str(s) => s.clone(),
                        _ => panic!("Invoke: expected Str constant for method name"),
                    };
                    let recv_slot = self.stack.len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;
                    let method = match &self.stack[recv_slot] {
                        VmValue::Instance { methods, .. } => {
                            methods.get(&method_name).cloned()
                                .ok_or_else(|| VmError::TypeError {
                                    message: format!("method '{}' not found", method_name),
                                    line,
                                })?
                        }
                        other => return Err(VmError::TypeError {
                            message: format!("cannot invoke '{}' on {}", method_name, other),
                            line,
                        }),
                    };
                    if method.function.arity != arg_count {
                        return Err(VmError::TypeError {
                            message: format!(
                                "method '{}' expects {} arg(s), got {}",
                                method_name, method.function.arity, arg_count
                            ),
                            line,
                        });
                    }
                    self.frames.push(CallFrame {
                        function: method.function,
                        upvalues: method.upvalues,
                        ip:       0,
                        base:     recv_slot,
                        block:    None,
                    });
                }

                OpCode::GetSelf => {
                    let base = self.frames.last().unwrap().base;
                    let val  = self.stack[base].clone();
                    self.stack.push(val);
                }

                // ── Block opcodes ─────────────────────────────────────────────

                OpCode::CallWithBlock(arg_count) => {
                    // Stack: [..., fn_or_closure, arg0, …, argN-1, block_closure]
                    let block_val = self.pop()?;
                    let block = match block_val {
                        VmValue::Closure { function, upvalues } =>
                            Some(VmMethod { function, upvalues }),
                        VmValue::Nil => None,
                        other => return Err(VmError::TypeError {
                            message: format!("block must be a closure, got {}", other),
                            line,
                        }),
                    };
                    let fn_slot = self.stack.len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;
                    let (function, upvalues) = match &self.stack[fn_slot] {
                        VmValue::Function(f)  => (f.clone(), vec![]),
                        VmValue::Closure { function, upvalues } =>
                            (function.clone(), upvalues.clone()),
                        other => return Err(VmError::TypeError {
                            message: format!("'{}' is not callable", other), line,
                        }),
                    };
                    if function.arity != arg_count {
                        return Err(VmError::TypeError {
                            message: format!(
                                "'{}' expects {} arg(s), got {}",
                                function.name, function.arity, arg_count
                            ),
                            line,
                        });
                    }
                    self.frames.push(CallFrame {
                        function, upvalues, ip: 0, base: fn_slot, block,
                    });
                }

                OpCode::InvokeWithBlock(name_idx, arg_count) => {
                    // Stack: [..., receiver, arg0, …, argN-1, block_closure]
                    let block_val = self.pop()?;
                    let block = match block_val {
                        VmValue::Closure { function, upvalues } =>
                            Some(VmMethod { function, upvalues }),
                        VmValue::Nil => None,
                        other => return Err(VmError::TypeError {
                            message: format!("block must be a closure, got {}", other),
                            line,
                        }),
                    };
                    let method_name = match &self.frames.last().unwrap().function.chunk.constants[name_idx] {
                        Constant::Str(s) => s.clone(),
                        _ => panic!("InvokeWithBlock: expected Str constant"),
                    };
                    let recv_slot = self.stack.len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;
                    let method = match &self.stack[recv_slot] {
                        VmValue::Instance { methods, .. } =>
                            methods.get(&method_name).cloned()
                                .ok_or_else(|| VmError::TypeError {
                                    message: format!("method '{}' not found", method_name),
                                    line,
                                })?,
                        other => return Err(VmError::TypeError {
                            message: format!("cannot invoke '{}' on {}", method_name, other),
                            line,
                        }),
                    };
                    if method.function.arity != arg_count {
                        return Err(VmError::TypeError {
                            message: format!(
                                "method '{}' expects {} arg(s), got {}",
                                method_name, method.function.arity, arg_count
                            ),
                            line,
                        });
                    }
                    self.frames.push(CallFrame {
                        function: method.function,
                        upvalues: method.upvalues,
                        ip: 0,
                        base: recv_slot,
                        block,
                    });
                }

                OpCode::Yield(arg_count) => {
                    let block = self.frames.last()
                        .and_then(|f| f.block.clone())
                        .ok_or_else(|| VmError::TypeError {
                            message: "yield called without a block".into(), line,
                        })?;
                    if block.function.arity != arg_count {
                        return Err(VmError::TypeError {
                            message: format!(
                                "block expects {} arg(s), got {}",
                                block.function.arity, arg_count
                            ),
                            line,
                        });
                    }
                    // Push the block closure as slot 0 of the new frame, then
                    // the args that are already on the top of the stack slide in
                    // as slots 1..arg_count.  We need to insert the closure
                    // below the args already on the stack.
                    let args_start = self.stack.len() - arg_count;
                    self.stack.insert(args_start, VmValue::Closure {
                        function: block.function.clone(),
                        upvalues: block.upvalues.clone(),
                    });
                    self.frames.push(CallFrame {
                        function: block.function,
                        upvalues: block.upvalues,
                        ip:    0,
                        base:  args_start,
                        block: None,
                    });
                }

                OpCode::Print => {
                    let val = self.pop()?;
                    println!("{}", val);
                    self.stack.push(VmValue::Nil);
                }

                OpCode::Call(arg_count) => {
                    // Stack: [..., fn_or_closure, arg0, …, argN-1]
                    let fn_slot = self.stack.len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;

                    let (function, upvalues) = match &self.stack[fn_slot] {
                        VmValue::Function(f) => (f.clone(), vec![]),
                        VmValue::Closure { function, upvalues } => {
                            (function.clone(), upvalues.clone())
                        }
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
                    self.frames.push(CallFrame { function, upvalues, ip: 0, base, block: None });
                }

                OpCode::Return => {
                    let return_val = self.stack.pop();
                    let frame      = self.frames.pop().unwrap();

                    // Close every upvalue that points into the returning frame.
                    self.close_upvalues_above(frame.base);

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

                OpCode::Equal    => { let (a, b) = self.pop2()?; self.stack.push(VmValue::Bool(a == b)); }
                OpCode::NotEqual => { let (a, b) = self.pop2()?; self.stack.push(VmValue::Bool(a != b)); }

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

    // ── Upvalue helpers ───────────────────────────────────────────────────────

    /// Return an open upvalue for `stack_idx`, reusing an existing one if
    /// present (so all closures that capture the same slot share one cell).
    fn capture_upvalue(&mut self, stack_idx: usize) -> Upvalue {
        if let Some(uv) = self.open_upvalues.iter().find(|uv| {
            matches!(*uv.0.borrow(), UpvalueState::Open(i) if i == stack_idx)
        }) {
            return uv.clone();
        }
        let uv = Upvalue::new_open(stack_idx);
        self.open_upvalues.push(uv.clone());
        uv
    }

    /// Close all open upvalues whose stack index is >= `first_slot` by copying
    /// the current stack value into the upvalue cell.
    fn close_upvalues_above(&mut self, first_slot: usize) {
        for uv in &self.open_upvalues {
            let mut state = uv.0.borrow_mut();
            if let UpvalueState::Open(idx) = *state {
                if idx >= first_slot {
                    let val = self.stack[idx].clone();
                    *state = UpvalueState::Closed(val);
                }
            }
        }
        self.open_upvalues
            .retain(|uv| matches!(*uv.0.borrow(), UpvalueState::Open(_)));
    }

    // ── Stack helpers ─────────────────────────────────────────────────────────

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{Chunk, Constant, Function, OpCode};

    fn run(chunk: Chunk) -> Result<Option<VmValue>, VmError> {
        let f = Rc::new(Function { name: String::new(), arity: 0, chunk, upvalue_defs: vec![] });
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
