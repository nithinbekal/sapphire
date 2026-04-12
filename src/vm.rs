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
    /// A compiled class: holds the static field list (with defaults) and the method table.
    Class {
        name:          String,
        #[allow(dead_code)]
        superclass:    Option<String>,
        fields:        Vec<(String, VmValue)>,
        methods:       Rc<HashMap<String, VmMethod>>,
        class_methods: Rc<HashMap<String, VmMethod>>,
    },
    /// A live instance of a class.
    Instance {
        class_name: String,
        fields:     Rc<RefCell<HashMap<String, VmValue>>>,
        methods:    Rc<HashMap<String, VmMethod>>,
    },
}

/// A compiled method: a function together with any upvalues it closed over,
/// and the name of the class that originally defined it (used by `super`).
#[derive(Debug, Clone)]
pub struct VmMethod {
    pub function:   Rc<Function>,
    pub upvalues:   Vec<Upvalue>,
    /// Name of the class this method was defined in; empty for block closures.
    pub defined_in: String,
    pub private:    bool,
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
    TypeError    { message: String, line: u32 },
    /// `raise val` — propagates until caught by a `Begin` handler.
    Raised(VmValue),
    /// `break val` inside a block — unwinds to the enclosing call-with-block.
    Break(VmValue),
    /// `next val` inside a block — skips to the next `yield`.
    #[allow(dead_code)]
    Next(VmValue),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::StackUnderflow => write!(f, "stack underflow"),
            VmError::TypeError { message, line } =>
                write!(f, "[line {}] type error: {}", line, message),
            VmError::Raised(v)  => write!(f, "uncaught raise: {}", v),
            VmError::Break(v)   => write!(f, "break outside block: {}", v),
            VmError::Next(v)    => write!(f, "next outside block: {}", v),
        }
    }
}

// ── Call frame ────────────────────────────────────────────────────────────────

/// Rescue handler registered by `BeginRescue`; popped by `PopRescue`.
#[derive(Clone, Copy)]
struct RescueInfo {
    handler_ip:       usize,
    rescue_var_slot:  usize, // usize::MAX means no variable
    stack_height:     usize, // stack depth at BeginRescue time (for cleanup)
}

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
    /// True if this frame was pushed by `CallWithBlock`/`InvokeWithBlock`.
    /// Signals `break` to stop unwinding here.
    is_block_caller: bool,
    /// Active rescue handlers within this frame (push on BeginRescue, pop on PopRescue).
    rescues: Vec<RescueInfo>,
    /// The class that defines the method running in this frame; None for
    /// non-method frames (plain functions, blocks).  Used by `SuperInvoke`.
    class_name: Option<String>,
}

// ── VM ────────────────────────────────────────────────────────────────────────

/// Per-class metadata stored by DefClass.
///
/// `methods` holds the *merged* (inherited + own) method table — the same map
/// that lives on `VmValue::Class`.  Using the merged table means:
///
/// * Primitive dispatch (`Invoke` on Int/Str/…) finds inherited Object methods.
/// * `SuperInvoke` looks up `classes[super_name].methods` and gets the
///   correct merged map for that ancestor level.
struct ClassEntry {
    superclass:    Option<String>,
    /// The merged (inherited + own) field list with default values.
    fields:        Vec<(String, VmValue)>,
    /// Merged (inherited + own) instance methods.
    methods:       Rc<HashMap<String, VmMethod>>,
    /// Merged (inherited + own) class methods.
    class_methods: Rc<HashMap<String, VmMethod>>,
}

pub struct Vm {
    frames:        Vec<CallFrame>,
    stack:         Vec<VmValue>,
    /// All upvalues that still point into the live stack (open upvalues),
    /// kept so we can close them when a frame exits.
    open_upvalues: Vec<Upvalue>,
    /// Registry of every class defined so far, keyed by name.  Stores each
    /// class's own (non-merged) methods so SuperInvoke can dispatch to them.
    classes: HashMap<String, ClassEntry>,
    /// Global variable store used by the REPL to persist values across calls.
    pub globals: HashMap<String, VmValue>,
}

impl Vm {
    pub fn new(function: Rc<Function>) -> Self {
        let frame = CallFrame {
            function, upvalues: vec![], ip: 0, base: 0,
            block: None, is_block_caller: false, rescues: vec![],
            class_name: None,
        };
        Vm { frames: vec![frame], stack: Vec::new(), open_upvalues: Vec::new(), classes: HashMap::new(), globals: HashMap::new() }
    }

    /// Create an empty VM with no initial frame, for use in the REPL.
    /// Call `load_stdlib()` before evaluating any user code.
    pub fn new_repl() -> Self {
        Vm { frames: vec![], stack: Vec::new(), open_upvalues: Vec::new(), classes: HashMap::new(), globals: HashMap::new() }
    }

    /// Evaluate a compiled function in the current VM context and return its result.
    /// Used by the REPL to run each input snippet while preserving global state.
    pub fn eval(&mut self, func: Rc<Function>) -> Result<Option<VmValue>, VmError> {
        let min_depth = self.frames.len();
        let base = self.stack.len();
        self.stack.push(VmValue::Function(func.clone()));
        self.frames.push(CallFrame {
            function: func, upvalues: vec![], ip: 0, base,
            block: None, is_block_caller: false, rescues: vec![],
            class_name: None,
        });
        let result = self.run_inner(min_depth);
        self.stack.truncate(base);
        result
    }

    pub fn run(&mut self) -> Result<Option<VmValue>, VmError> {
        self.run_inner(0)
    }

    /// Run a compiled top-level function in the current VM context (for stdlib loading).
    fn run_extra(&mut self, func: Rc<Function>) -> Result<(), VmError> {
        let min_depth = self.frames.len();
        let base = self.stack.len();
        self.stack.push(VmValue::Function(func.clone()));
        self.frames.push(CallFrame {
            function: func, upvalues: vec![], ip: 0, base,
            block: None, is_block_caller: false, rescues: vec![],
            class_name: None,
        });
        self.run_inner(min_depth)?;
        self.stack.truncate(base);
        Ok(())
    }

    /// Compile and execute the stdlib Sapphire files to populate the class registry.
    pub fn load_stdlib(&mut self) -> Result<(), VmError> {
        const SOURCES: &[(&str, &str)] = &[
            ("stdlib/object.spr", include_str!("../stdlib/object.spr")),
            ("stdlib/nil.spr",    include_str!("../stdlib/nil.spr")),
            ("stdlib/num.spr",    include_str!("../stdlib/num.spr")),
            ("stdlib/int.spr",    include_str!("../stdlib/int.spr")),
            ("stdlib/float.spr",  include_str!("../stdlib/float.spr")),
            ("stdlib/string.spr", include_str!("../stdlib/string.spr")),
            ("stdlib/bool.spr",   include_str!("../stdlib/bool.spr")),
            ("stdlib/list.spr",   include_str!("../stdlib/list.spr")),
            ("stdlib/map.spr",    include_str!("../stdlib/map.spr")),
        ];
        for (name, src) in SOURCES {
            let tokens = crate::lexer::Lexer::new(src).scan_tokens();
            let stmts = crate::parser::Parser::new(tokens).parse()
                .map_err(|e| VmError::TypeError { message: format!("{}: {}", name, e), line: 0 })?;
            let func = crate::compiler::compile(&stmts)
                .map_err(|e| VmError::TypeError { message: format!("{}: {}", name, e), line: 0 })?;
            self.run_extra(func)?;
        }
        Ok(())
    }

    fn run_inner(&mut self, min_depth: usize) -> Result<Option<VmValue>, VmError> {
        loop {
            if self.frames.len() <= min_depth {
                return Ok(None);
            }
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
                    let (class_name, superclass_name, own_fields, class_method_names, method_names, private_methods) = {
                        let consts = &self.frames.last().unwrap().function.chunk.constants;
                        match &consts[desc_idx] {
                            Constant::ClassDesc { name, superclass, field_names, field_defaults, method_names, private_methods, class_method_names } => {
                                let own_fields: Vec<(String, VmValue)> = field_names.iter()
                                    .zip(field_defaults.iter())
                                    .map(|(n, d)| (n.clone(), d.as_ref().map(VmValue::from).unwrap_or(VmValue::Nil)))
                                    .collect();
                                (name.clone(), superclass.clone(), own_fields, class_method_names.clone(), method_names.clone(), private_methods.clone())
                            }
                            _ => panic!("DefClass: expected ClassDesc constant"),
                        }
                    };
                    // Drain class method closures first (emitted first by compiler).
                    let n_class = class_method_names.len();
                    let class_start = self.stack.len()
                        .checked_sub(n_class + method_names.len())
                        .ok_or(VmError::StackUnderflow)?;
                    let all_closures: Vec<VmValue> = self.stack.drain(class_start..).collect();
                    let (class_closures, instance_closures) = all_closures.split_at(n_class);

                    let mut own_class_methods: HashMap<String, VmMethod> = HashMap::new();
                    for (mname, closure) in class_method_names.iter().zip(class_closures) {
                        match closure {
                            VmValue::Closure { function, upvalues } => {
                                own_class_methods.insert(mname.clone(), VmMethod {
                                    function: function.clone(), upvalues: upvalues.clone(),
                                    defined_in: class_name.clone(), private: false,
                                });
                            }
                            _ => panic!("DefClass: class method is not a closure"),
                        }
                    }
                    let mut own_methods: HashMap<String, VmMethod> = HashMap::new();
                    for (mname, closure) in method_names.iter().zip(instance_closures) {
                        match closure {
                            VmValue::Closure { function, upvalues } => {
                                let private = private_methods.contains(mname);
                                own_methods.insert(mname.clone(), VmMethod {
                                    function: function.clone(), upvalues: upvalues.clone(),
                                    defined_in: class_name.clone(), private,
                                });
                            }
                            _ => panic!("DefClass: method is not a closure"),
                        }
                    }
                    // If no explicit superclass and Object is loaded, inherit from it.
                    let effective_super = superclass_name.clone().or_else(|| {
                        if class_name != "Object" && self.classes.contains_key("Object") {
                            Some("Object".to_string())
                        } else {
                            None
                        }
                    });
                    // Merge inherited fields, instance methods, and class methods.
                    let (merged_fields, merged_methods, merged_class_methods) = if let Some(ref sname) = effective_super {
                        let (parent_fields, parent_methods, parent_class_methods) = match self.classes.get(sname) {
                            Some(entry) => (entry.fields.clone(), (*entry.methods).clone(), (*entry.class_methods).clone()),
                            None => return Err(VmError::TypeError {
                                message: format!("superclass '{}' not found", sname),
                                line,
                            }),
                        };
                        let mut mf = parent_fields;
                        mf.extend(own_fields);
                        let mut mm = parent_methods;
                        mm.extend(own_methods);
                        let mut mc = parent_class_methods;
                        mc.extend(own_class_methods);
                        (mf, mm, mc)
                    } else {
                        (own_fields, own_methods, own_class_methods)
                    };
                    let merged_rc         = Rc::new(merged_methods);
                    let merged_class_rc   = Rc::new(merged_class_methods);
                    self.classes.insert(class_name.clone(), ClassEntry {
                        superclass:    effective_super.clone(),
                        fields:        merged_fields.clone(),
                        methods:       merged_rc.clone(),
                        class_methods: merged_class_rc.clone(),
                    });
                    self.stack.push(VmValue::Class {
                        name:          class_name,
                        superclass:    effective_super,
                        fields:        merged_fields,
                        methods:       merged_rc,
                        class_methods: merged_class_rc,
                    });
                }

                OpCode::NewInstance(n_pairs) => {
                    // Stack: [class, name0, val0, …, nameN, valN]
                    let base = self.stack.len()
                        .checked_sub(1 + n_pairs * 2)
                        .ok_or(VmError::StackUnderflow)?;
                    let (class_name, field_decls, methods) = match &self.stack[base] {
                        VmValue::Class { name, fields, methods, .. } =>
                            (name.clone(), fields.clone(), methods.clone()),
                        other => return Err(VmError::TypeError {
                            message: format!("'{}' is not a class", other),
                            line,
                        }),
                    };
                    // Initialise fields to their declared defaults (or nil if none).
                    let mut instance_fields: HashMap<String, VmValue> =
                        field_decls.iter().map(|(n, default)| (n.clone(), default.clone())).collect();
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
                        // For primitives, treat `obj.name` as a zero-arg method call.
                        ref other => {
                            match try_native_method(other, &name, &[], line) {
                                Some(Ok(result)) => { self.stack.push(result); }
                                Some(Err(e)) => return Err(e),
                                None => {
                                    // Try compiled stdlib methods from class registry.
                                    let method = primitive_class_name(other)
                                        .and_then(|cls| self.classes.get(cls))
                                        .and_then(|entry| entry.methods.get(&name).cloned());
                                    match method {
                                        Some(m) => {
                                            let recv_slot = self.stack.len();
                                            self.stack.push(other.clone());
                                            let class_name = Some(m.defined_in.clone());
                                            self.frames.push(CallFrame {
                                                function: m.function, upvalues: m.upvalues,
                                                ip: 0, base: recv_slot,
                                                block: None, is_block_caller: false, rescues: vec![],
                                                class_name,
                                            });
                                        }
                                        None => return Err(VmError::TypeError {
                                            message: format!("cannot get field '{}' on {}", name, other),
                                            line,
                                        }),
                                    }
                                }
                            }
                        }
                    }
                }

                OpCode::GetFieldSafe(idx) => {
                    let name = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Str(s) => s.clone(),
                        _ => panic!("GetFieldSafe: expected Str constant"),
                    };
                    let obj = self.pop()?;
                    match obj {
                        VmValue::Nil => {
                            self.stack.push(VmValue::Nil);
                        }
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

                    if method_name == "is_a?" && arg_count == 1 {
                        let recv = self.stack[recv_slot].clone();
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        let result = invoke_is_a(&self.classes, &recv, &args, line)?;
                        self.stack.truncate(recv_slot);
                        self.stack.push(result);
                        continue;
                    }

                    // Lambda `.call(args)` — invoke the closure as a new frame.
                    if method_name == "call" {
                        if let VmValue::Closure { function, upvalues } = self.stack[recv_slot].clone() {
                            if function.arity != arg_count {
                                return Err(VmError::TypeError {
                                    message: format!(
                                        "lambda expects {} argument(s), got {}",
                                        function.arity, arg_count
                                    ),
                                    line,
                                });
                            }
                            self.frames.push(CallFrame {
                                function,
                                upvalues,
                                ip: 0, base: recv_slot,
                                block: None, is_block_caller: false, rescues: vec![],
                                class_name: None,
                            });
                            continue;
                        }
                    }

                    // Class method dispatch: receiver is a Class value.
                    if let VmValue::Class { ref class_methods, .. } = self.stack[recv_slot].clone() {
                        let method = class_methods.get(&method_name).cloned()
                            .ok_or_else(|| VmError::TypeError {
                                message: format!("unknown class method '{}'", method_name),
                                line,
                            })?;
                        if method.function.arity != arg_count {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "class method '{}' expects {} arg(s), got {}",
                                    method_name, method.function.arity, arg_count
                                ),
                                line,
                            });
                        }
                        self.frames.push(CallFrame {
                            function:  method.function,
                            upvalues:  method.upvalues,
                            ip: 0, base: recv_slot,
                            block: None, is_block_caller: false, rescues: vec![],
                            class_name: Some(method.defined_in),
                        });
                        continue;
                    }

                    // Try native dispatch for non-Instance types.
                    let is_instance = matches!(&self.stack[recv_slot], VmValue::Instance { .. });
                    if !is_instance {
                        let recv = self.stack[recv_slot].clone();
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        // Try Rust native method first; if not found try compiled stdlib.
                        match try_native_method(&recv, &method_name, &args, line) {
                            Some(Ok(result)) => {
                                self.stack.truncate(recv_slot);
                                self.stack.push(result);
                                continue;
                            }
                            Some(Err(e)) => return Err(e),
                            None => {}
                        }
                        // Native didn't handle it — look in the class registry.
                        let method = primitive_class_name(&recv)
                            .and_then(|cls| self.classes.get(cls))
                            .and_then(|entry| entry.methods.get(&method_name).cloned());
                        match method {
                            Some(m) => {
                                if m.private {
                                    let caller_class = self.frames.last().and_then(|f| f.class_name.as_deref()).unwrap_or("");
                                    if caller_class != m.defined_in {
                                        return Err(VmError::TypeError {
                                            message: format!("private method '{}' called from outside class", method_name),
                                            line,
                                        });
                                    }
                                }
                                if m.function.arity != arg_count {
                                    return Err(VmError::TypeError {
                                        message: format!(
                                            "method '{}' expects {} arg(s), got {}",
                                            method_name, m.function.arity, arg_count
                                        ),
                                        line,
                                    });
                                }
                                // recv and args are already on the stack at recv_slot..
                                // so the new frame's slot 0 = recv, slots 1.. = args.
                                let class_name = Some(m.defined_in.clone());
                                self.frames.push(CallFrame {
                                    function: m.function,
                                    upvalues: m.upvalues,
                                    ip: 0, base: recv_slot,
                                    block: None, is_block_caller: false, rescues: vec![],
                                    class_name,
                                });
                                continue;
                            }
                            None => return Err(VmError::TypeError {
                                message: format!("'{}' has no method '{}'", recv, method_name),
                                line,
                            }),
                        }
                    }

                    let method = match &self.stack[recv_slot] {
                        VmValue::Instance { methods, .. } => {
                            methods.get(&method_name).cloned()
                                .ok_or_else(|| VmError::TypeError {
                                    message: format!("method '{}' not found", method_name),
                                    line,
                                })?
                        }
                        _ => unreachable!(),
                    };
                    if method.private {
                        let caller_class = self.frames.last().and_then(|f| f.class_name.as_deref()).unwrap_or("");
                        if caller_class != method.defined_in {
                            return Err(VmError::TypeError {
                                message: format!("private method '{}' called from outside class", method_name),
                                line,
                            });
                        }
                    }
                    if method.function.arity != arg_count {
                        return Err(VmError::TypeError {
                            message: format!(
                                "method '{}' expects {} arg(s), got {}",
                                method_name, method.function.arity, arg_count
                            ),
                            line,
                        });
                    }
                    let class_name = Some(method.defined_in.clone());
                    self.frames.push(CallFrame {
                        function: method.function,
                        upvalues: method.upvalues,
                        ip: 0, base: recv_slot,
                        block: None, is_block_caller: false, rescues: vec![],
                        class_name,
                    });
                }

                OpCode::SuperInvoke(name_idx, arg_count) => {
                    let method_name = match &self.frames.last().unwrap().function.chunk.constants[name_idx] {
                        Constant::Str(s) => s.clone(),
                        _ => panic!("SuperInvoke: expected Str constant"),
                    };
                    let current_class = self.frames.last().unwrap().class_name.clone()
                        .ok_or_else(|| VmError::TypeError {
                            message: "super used outside of a method".into(), line,
                        })?;
                    let super_name = match self.classes.get(&current_class) {
                        Some(ClassEntry { superclass: Some(s), .. }) => s.clone(),
                        Some(ClassEntry { superclass: None,    .. }) => return Err(VmError::TypeError {
                            message: format!("'{}' has no superclass", current_class), line,
                        }),
                        None => return Err(VmError::TypeError {
                            message: format!("class '{}' not in registry", current_class), line,
                        }),
                    };
                    let method = match self.classes.get(&super_name) {
                        Some(entry) => entry.methods.get(&method_name).cloned()
                            .ok_or_else(|| VmError::TypeError {
                                message: format!("superclass '{}' has no method '{}'", super_name, method_name),
                                line,
                            })?,
                        None => return Err(VmError::TypeError {
                            message: format!("superclass '{}' not in registry", super_name), line,
                        }),
                    };
                    let recv_slot = self.stack.len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;
                    if method.function.arity != arg_count {
                        return Err(VmError::TypeError {
                            message: format!(
                                "method '{}' expects {} arg(s), got {}",
                                method_name, method.function.arity, arg_count
                            ),
                            line,
                        });
                    }
                    // Use super_name as the class for the new frame so chained
                    // super calls continue up the inheritance chain correctly.
                    self.frames.push(CallFrame {
                        function: method.function,
                        upvalues: method.upvalues,
                        ip: 0, base: recv_slot,
                        block: None, is_block_caller: false, rescues: vec![],
                        class_name: Some(super_name),
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
                            Some(VmMethod { function, upvalues, defined_in: String::new(), private: false }),
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
                        function, upvalues, ip: 0, base: fn_slot,
                        block, is_block_caller: true, rescues: vec![],
                        class_name: None,
                    });
                }

                OpCode::InvokeWithBlock(name_idx, arg_count) => {
                    // Stack: [..., receiver, arg0, …, argN-1, block_closure]
                    let block_val = self.pop()?;
                    let block = match block_val {
                        VmValue::Closure { function, upvalues } =>
                            Some(VmMethod { function, upvalues, defined_in: String::new(), private: false }),
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

                    // Try native block dispatch for non-Instance types; fall back to
                    // compiled stdlib methods in the class registry.
                    let is_instance = matches!(&self.stack[recv_slot], VmValue::Instance { .. });
                    if !is_instance {
                        let recv = self.stack[recv_slot].clone();
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        // Peek at whether native dispatch handles this method.
                        let native_result = self.dispatch_native_block_method(
                            &recv, &method_name, &args, block.clone(), line,
                        );
                        let is_native_miss = matches!(&native_result,
                            Err(VmError::TypeError { message, .. })
                            if message.contains("has no block method") || message.contains("requires a block")
                        );
                        if !is_native_miss {
                            self.stack.truncate(recv_slot);
                            self.stack.push(native_result?);
                            continue;
                        }
                        // Native didn't handle it — try the class registry.
                        let method = primitive_class_name(&recv)
                            .and_then(|cls| self.classes.get(cls))
                            .and_then(|entry| entry.methods.get(&method_name).cloned());
                        match method {
                            Some(m) => {
                                // Stack is still [..., recv, args...]; leave it for the frame.
                                let class_name = Some(m.defined_in.clone());
                                self.frames.push(CallFrame {
                                    function: m.function, upvalues: m.upvalues,
                                    ip: 0, base: recv_slot,
                                    block, is_block_caller: true, rescues: vec![],
                                    class_name,
                                });
                                continue;
                            }
                            None => {
                                return Err(VmError::TypeError {
                                    message: format!(
                                        "no method '{}' on {}",
                                        method_name, recv
                                    ),
                                    line,
                                });
                            }
                        }
                    }

                    let method = match &self.stack[recv_slot] {
                        VmValue::Instance { methods, .. } =>
                            methods.get(&method_name).cloned()
                                .ok_or_else(|| VmError::TypeError {
                                    message: format!("method '{}' not found", method_name),
                                    line,
                                })?,
                        _ => unreachable!(),
                    };
                    if method.private {
                        let caller_class = self.frames.last().and_then(|f| f.class_name.as_deref()).unwrap_or("");
                        if caller_class != method.defined_in {
                            return Err(VmError::TypeError {
                                message: format!("private method '{}' called from outside class", method_name),
                                line,
                            });
                        }
                    }
                    if method.function.arity != arg_count {
                        return Err(VmError::TypeError {
                            message: format!(
                                "method '{}' expects {} arg(s), got {}",
                                method_name, method.function.arity, arg_count
                            ),
                            line,
                        });
                    }
                    let class_name = Some(method.defined_in.clone());
                    self.frames.push(CallFrame {
                        function: method.function,
                        upvalues: method.upvalues,
                        ip: 0, base: recv_slot,
                        block, is_block_caller: true, rescues: vec![],
                        class_name,
                    });
                }

                OpCode::Yield(arg_count) => {
                    // Walk up the frame stack to find the nearest block — this allows
                    // `yield` inside an inner block to call back to the enclosing method's block.
                    let block = self.frames.iter().rev()
                        .find_map(|f| f.block.clone())
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
                        block: None, is_block_caller: false, rescues: vec![],
                        class_name: None,
                    });
                }

                // ── Exception-like control flow ───────────────────────────────

                OpCode::Raise => {
                    let val = self.pop()?;
                    self.raise_value(val)?;
                }

                OpCode::Break => {
                    let val = self.pop()?;
                    // Unwind until we reach a frame created by CallWithBlock /
                    // InvokeWithBlock, then return the break value from IT too.
                    loop {
                        if let Some(frame) = self.frames.last() {
                            let is_caller = frame.is_block_caller;
                            let base      = frame.base;
                            self.close_upvalues_above(base);
                            self.frames.pop();
                            self.stack.truncate(base);
                            if is_caller {
                                // Push break value as the result of the call-with-block.
                                self.stack.push(val);
                                break;
                            }
                        } else {
                            return Err(VmError::Break(val));
                        }
                    }
                }

                OpCode::Next => {
                    // Return from the current (block) frame immediately with `val`.
                    let val   = self.pop()?;
                    let frame = self.frames.pop().unwrap();
                    self.close_upvalues_above(frame.base);
                    if self.frames.is_empty() {
                        return Ok(Some(val));
                    }
                    self.stack.truncate(frame.base);
                    self.stack.push(val);
                }

                OpCode::BeginRescue { handler_offset, rescue_var_slot } => {
                    let handler_ip   = self.frames.last().unwrap().ip + handler_offset;
                    let stack_height = self.stack.len();
                    self.frames.last_mut().unwrap().rescues.push(RescueInfo {
                        handler_ip, rescue_var_slot, stack_height,
                    });
                }

                OpCode::PopRescue => {
                    self.frames.last_mut().unwrap().rescues.pop();
                }

                OpCode::Print => {
                    let val = self.pop()?;
                    println!("{}", val);
                    self.stack.push(val);
                }

                OpCode::GetGlobal(idx) => {
                    let name = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Str(s) => s.clone(),
                        _ => return Err(VmError::TypeError { message: "GetGlobal: expected string constant".to_string(), line }),
                    };
                    let val = self.globals.get(&name).cloned().ok_or_else(|| VmError::TypeError {
                        message: format!("undefined variable '{}'", name),
                        line,
                    })?;
                    self.stack.push(val);
                }

                OpCode::SetGlobal(idx) => {
                    let name = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Str(s) => s.clone(),
                        _ => return Err(VmError::TypeError { message: "SetGlobal: expected string constant".to_string(), line }),
                    };
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?.clone();
                    self.globals.insert(name, val);
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
                    self.frames.push(CallFrame {
                        function, upvalues, ip: 0, base,
                        block: None, is_block_caller: false, rescues: vec![],
                        class_name: None,
                    });
                }

                OpCode::Return => {
                    let return_val = self.stack.pop();
                    let frame      = self.frames.pop().unwrap();

                    // Close every upvalue that points into the returning frame.
                    self.close_upvalues_above(frame.base);

                    if self.frames.len() <= min_depth {
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
                    let result = if let (VmValue::Str(x), VmValue::Str(y)) = (&a, &b) {
                        VmValue::Str(format!("{}{}", x, y))
                    } else {
                        numeric_binop(&a, &b, line, "add", |x, y| x + y, |x, y| x + y)?
                    };
                    self.stack.push(result);
                }
                OpCode::Sub => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(numeric_binop(&a, &b, line, "subtract", |x, y| x - y, |x, y| x - y)?);
                }
                OpCode::Mul => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(numeric_binop(&a, &b, line, "multiply", |x, y| x * y, |x, y| x * y)?);
                }
                OpCode::Div => {
                    let (a, b) = self.pop2()?;
                    let is_zero = matches!(&b, VmValue::Int(0)) || matches!(&b, VmValue::Float(f) if *f == 0.0);
                    if is_zero {
                        self.raise_value(VmValue::Str("division by zero".into()))?;
                        continue;
                    }
                    self.stack.push(numeric_binop(&a, &b, line, "divide", |x, y| x / y, |x, y| x / y)?);
                }
                OpCode::Mod => {
                    let (a, b) = self.pop2()?;
                    let is_zero = matches!(&b, VmValue::Int(0)) || matches!(&b, VmValue::Float(f) if *f == 0.0);
                    if is_zero {
                        self.raise_value(VmValue::Str("division by zero".into()))?;
                        continue;
                    }
                    self.stack.push(numeric_binop(&a, &b, line, "modulo", |x, y| x % y, |x, y| x % y)?);
                }

                OpCode::Equal    => { let (a, b) = self.pop2()?; self.stack.push(VmValue::Bool(a == b)); }
                OpCode::NotEqual => { let (a, b) = self.pop2()?; self.stack.push(VmValue::Bool(a != b)); }

                OpCode::Len => {
                    let val = self.pop()?;
                    let n = match &val {
                        VmValue::List(v) => v.borrow().len() as i64,
                        VmValue::Map(m)  => m.borrow().len() as i64,
                        VmValue::Str(s)  => s.chars().count() as i64,
                        VmValue::Range { from, to } => (to - from).max(0),
                        other => return Err(VmError::TypeError {
                            message: format!("len() not supported for {}", other), line,
                        }),
                    };
                    self.stack.push(VmValue::Int(n));
                }

                OpCode::MapKeys => {
                    let val = self.pop()?;
                    let mut keys = match val {
                        VmValue::Map(m) => m.borrow().keys().cloned().collect::<Vec<_>>(),
                        other => return Err(VmError::TypeError {
                            message: format!("map_keys() not supported for {}", other), line,
                        }),
                    };
                    keys.sort();
                    let list = keys.into_iter().map(VmValue::Str).collect();
                    self.stack.push(VmValue::List(Rc::new(RefCell::new(list))));
                }

                OpCode::RangeFrom => {
                    let val = self.pop()?;
                    match val {
                        VmValue::Range { from, .. } => self.stack.push(VmValue::Int(from)),
                        other => return Err(VmError::TypeError {
                            message: format!("range_from() not supported for {}", other), line,
                        }),
                    }
                }

                OpCode::RangeTo => {
                    let val = self.pop()?;
                    match val {
                        VmValue::Range { to, .. } => self.stack.push(VmValue::Int(to)),
                        other => return Err(VmError::TypeError {
                            message: format!("range_to() not supported for {}", other), line,
                        }),
                    }
                }

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

    // ── Native stdlib helpers ─────────────────────────────────────────────────

    /// Call a block (closure) with `args`, running until it returns.
    fn call_block(&mut self, block: &VmMethod, args: Vec<VmValue>) -> Result<VmValue, VmError> {
        let min_depth = self.frames.len();
        let base = self.stack.len();
        self.stack.push(VmValue::Closure {
            function: block.function.clone(),
            upvalues: block.upvalues.clone(),
        });
        for arg in args {
            self.stack.push(arg);
        }
        self.frames.push(CallFrame {
            function: block.function.clone(),
            upvalues: block.upvalues.clone(),
            ip: 0, base,
            block: None, is_block_caller: false, rescues: vec![],
            class_name: None,
        });
        // run_inner stops when the block's frame returns (frames.len() drops to min_depth)
        // but does NOT truncate the stack back to `base` — we must do that here.
        let result = self.run_inner(min_depth);
        self.stack.truncate(base);
        result.map(|v| v.unwrap_or(VmValue::Nil))
    }

    /// Native dispatch for block-taking methods (each, map, times, …).
    fn dispatch_native_block_method(
        &mut self,
        recv: &VmValue,
        name: &str,
        args: &[VmValue],
        block: Option<VmMethod>,
        line: u32,
    ) -> Result<VmValue, VmError> {
        let blk = block.ok_or_else(|| VmError::TypeError {
            message: format!("'{}' requires a block", name),
            line,
        })?;
        match recv {
            VmValue::List(elems) => match name {
                "each" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    for item in items {
                        match self.call_block(&blk, vec![item]) {
                            Err(VmError::Next(_)) => continue,
                            Err(VmError::Break(v)) => return Ok(v),
                            Err(e) => return Err(e),
                            Ok(_) => {}
                        }
                    }
                    Ok(recv.clone())
                }
                "map" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    let mut out = Vec::with_capacity(items.len());
                    for item in items {
                        match self.call_block(&blk, vec![item]) {
                            Err(VmError::Next(v)) => out.push(v),
                            Err(VmError::Break(v)) => { out.push(v); break; }
                            Err(e) => return Err(e),
                            Ok(v) => out.push(v),
                        }
                    }
                    Ok(VmValue::List(Rc::new(RefCell::new(out))))
                }
                "select" | "filter" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    let mut out = Vec::new();
                    for item in items {
                        match self.call_block(&blk, vec![item.clone()]) {
                            Err(VmError::Break(_)) => break,
                            Err(e) => return Err(e),
                            Ok(v) if !is_falsy(&v) => out.push(item),
                            Ok(_) => {}
                        }
                    }
                    Ok(VmValue::List(Rc::new(RefCell::new(out))))
                }
                "reject" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    let mut out = Vec::new();
                    for item in items {
                        match self.call_block(&blk, vec![item.clone()]) {
                            Err(VmError::Break(_)) => break,
                            Err(e) => return Err(e),
                            Ok(v) if is_falsy(&v) => out.push(item),
                            Ok(_) => {}
                        }
                    }
                    Ok(VmValue::List(Rc::new(RefCell::new(out))))
                }
                "any?" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    for item in items {
                        match self.call_block(&blk, vec![item]) {
                            Err(VmError::Break(_)) => return Ok(VmValue::Bool(false)),
                            Err(e) => return Err(e),
                            Ok(v) if !is_falsy(&v) => return Ok(VmValue::Bool(true)),
                            Ok(_) => {}
                        }
                    }
                    Ok(VmValue::Bool(false))
                }
                "all?" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    for item in items {
                        match self.call_block(&blk, vec![item]) {
                            Err(VmError::Break(_)) => return Ok(VmValue::Bool(true)),
                            Err(e) => return Err(e),
                            Ok(v) if is_falsy(&v) => return Ok(VmValue::Bool(false)),
                            Ok(_) => {}
                        }
                    }
                    Ok(VmValue::Bool(true))
                }
                "none?" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    for item in items {
                        match self.call_block(&blk, vec![item]) {
                            Err(VmError::Break(_)) => return Ok(VmValue::Bool(true)),
                            Err(e) => return Err(e),
                            Ok(v) if !is_falsy(&v) => return Ok(VmValue::Bool(false)),
                            Ok(_) => {}
                        }
                    }
                    Ok(VmValue::Bool(true))
                }
                "reduce" | "inject" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    let mut acc = if args.is_empty() {
                        items.first().cloned().unwrap_or(VmValue::Nil)
                    } else {
                        args[0].clone()
                    };
                    let skip = if args.is_empty() { 1 } else { 0 };
                    for item in items.into_iter().skip(skip) {
                        acc = self.call_block(&blk, vec![acc, item])?;
                    }
                    Ok(acc)
                }
                "each_with_index" => {
                    let items: Vec<VmValue> = elems.borrow().clone();
                    for (i, item) in items.into_iter().enumerate() {
                        match self.call_block(&blk, vec![item, VmValue::Int(i as i64)]) {
                            Err(VmError::Break(v)) => return Ok(v),
                            Err(e) => return Err(e),
                            Ok(_) => {}
                        }
                    }
                    Ok(recv.clone())
                }
                _ => Err(VmError::TypeError {
                    message: format!("List has no block method '{}'", name), line,
                }),
            },

            VmValue::Range { from, to } => {
                let (from, to) = (*from, *to);
                match name {
                    "each" => {
                        let mut i = from;
                        while i < to {
                            match self.call_block(&blk, vec![VmValue::Int(i)]) {
                                Err(VmError::Next(_)) => { i += 1; continue; }
                                Err(VmError::Break(v)) => return Ok(v),
                                Err(e) => return Err(e),
                                Ok(_) => {}
                            }
                            i += 1;
                        }
                        Ok(recv.clone())
                    }
                    "map" => {
                        let mut out = Vec::new();
                        for i in from..to {
                            out.push(self.call_block(&blk, vec![VmValue::Int(i)])?);
                        }
                        Ok(VmValue::List(Rc::new(RefCell::new(out))))
                    }
                    _ => Err(VmError::TypeError {
                        message: format!("Range has no block method '{}'", name), line,
                    }),
                }
            }

            VmValue::Int(n) => match name {
                "times" => {
                    let n = *n;
                    for i in 0..n {
                        match self.call_block(&blk, vec![VmValue::Int(i)]) {
                            Err(VmError::Next(_)) => continue,
                            Err(VmError::Break(v)) => return Ok(v),
                            Err(e) => return Err(e),
                            Ok(_) => {}
                        }
                    }
                    Ok(recv.clone())
                }
                "upto" => {
                    let from = *n;
                    let to = match args.first() {
                        Some(VmValue::Int(t)) => *t,
                        _ => return Err(VmError::TypeError { message: "upto expects an Int".into(), line }),
                    };
                    for i in from..=to {
                        match self.call_block(&blk, vec![VmValue::Int(i)]) {
                            Err(VmError::Break(v)) => return Ok(v),
                            Err(e) => return Err(e),
                            Ok(_) => {}
                        }
                    }
                    Ok(recv.clone())
                }
                _ => Err(VmError::TypeError {
                    message: format!("Int has no block method '{}'", name), line,
                }),
            },

            VmValue::Map(map) => match name {
                "each" => {
                    let pairs: Vec<(String, VmValue)> = map.borrow().iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    for (k, v) in pairs {
                        match self.call_block(&blk, vec![VmValue::Str(k), v]) {
                            Err(VmError::Break(val)) => return Ok(val),
                            Err(e) => return Err(e),
                            Ok(_) => {}
                        }
                    }
                    Ok(recv.clone())
                }
                "map" => {
                    let pairs: Vec<(String, VmValue)> = map.borrow().iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    let mut out = Vec::with_capacity(pairs.len());
                    for (k, v) in pairs {
                        out.push(self.call_block(&blk, vec![VmValue::Str(k), v])?);
                    }
                    Ok(VmValue::List(Rc::new(RefCell::new(out))))
                }
                _ => Err(VmError::TypeError {
                    message: format!("Map has no block method '{}'", name), line,
                }),
            },

            other => Err(VmError::TypeError {
                message: format!("'{}' does not support block method '{}'", other, name),
                line,
            }),
        }
    }

    // ── Stack helpers ─────────────────────────────────────────────────────────

    fn raise_value(&mut self, val: VmValue) -> Result<(), VmError> {
        loop {
            if let Some(frame) = self.frames.last_mut() {
                if let Some(info) = frame.rescues.pop() {
                    self.stack.truncate(info.stack_height);
                    if info.rescue_var_slot != usize::MAX {
                        let slot = self.frames.last().unwrap().base + info.rescue_var_slot;
                        while self.stack.len() <= slot {
                            self.stack.push(VmValue::Nil);
                        }
                        self.stack[slot] = val;
                    }
                    self.frames.last_mut().unwrap().ip = info.handler_ip;
                    return Ok(());
                }
                let base = frame.base;
                self.close_upvalues_above(base);
                self.frames.pop();
                self.stack.truncate(base);
            } else {
                return Err(VmError::Raised(val));
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

/// Dispatch a native (non-block) method call on a built-in type.
fn dispatch_native_method(
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };

    match recv {
        VmValue::Int(n) => match (name, args) {
            ("to_s",  [])                           => Ok(VmValue::Str(n.to_string())),
            ("to_f",  [])                           => Ok(VmValue::Float(*n as f64)),
            ("abs",   [])                           => Ok(VmValue::Int(n.abs())),
            ("even?", [])                           => Ok(VmValue::Bool(n % 2 == 0)),
            ("odd?",  [])                           => Ok(VmValue::Bool(n % 2 != 0)),
            ("zero?", [])                           => Ok(VmValue::Bool(*n == 0)),
            ("to_r",  [])                           => Ok(recv.clone()),
            ("pow",   [VmValue::Int(e)]) if *e >= 0 => Ok(VmValue::Int(n.pow(*e as u32))),
            ("max",   [VmValue::Int(other)])        => Ok(VmValue::Int(*n.max(other))),
            ("min",   [VmValue::Int(other)])        => Ok(VmValue::Int(*n.min(other))),
            _ => Err(type_err(&format!("Int has no method '{}'", name))),
        },

        VmValue::Float(n) => match (name, args) {
            ("to_s",  []) => Ok(VmValue::Str(if n.fract() == 0.0 {
                format!("{}.0", *n as i64)
            } else {
                format!("{}", n)
            })),
            ("to_i",  []) => Ok(VmValue::Int(*n as i64)),
            ("abs",   []) => Ok(VmValue::Float(n.abs())),
            ("round", []) => Ok(VmValue::Int(n.round() as i64)),
            ("floor", []) => Ok(VmValue::Int(n.floor() as i64)),
            ("ceil",  []) => Ok(VmValue::Int(n.ceil() as i64)),
            ("sqrt",  []) => Ok(VmValue::Float(n.sqrt())),
            ("nan?",  []) => Ok(VmValue::Bool(n.is_nan())),
            ("infinite?", []) => Ok(VmValue::Bool(n.is_infinite())),
            _ => Err(type_err(&format!("Float has no method '{}'", name))),
        },

        VmValue::Str(s) => match name {
            "size" | "length" if args.is_empty() => Ok(VmValue::Int(s.chars().count() as i64)),
            "upcase"   if args.is_empty() => Ok(VmValue::Str(s.to_uppercase())),
            "downcase" if args.is_empty() => Ok(VmValue::Str(s.to_lowercase())),
            "reverse"  if args.is_empty() => Ok(VmValue::Str(s.chars().rev().collect())),
            "strip" | "trim" if args.is_empty() => Ok(VmValue::Str(s.trim().to_string())),
            "chomp"    if args.is_empty() => Ok(VmValue::Str(s.trim_end_matches('\n').to_string())),
            "to_i"     if args.is_empty() => Ok(VmValue::Int(s.trim().parse::<i64>().unwrap_or(0))),
            "to_f"     if args.is_empty() => Ok(VmValue::Float(s.trim().parse::<f64>().unwrap_or(0.0))),
            "to_s"     if args.is_empty() => Ok(recv.clone()),
            "empty?"   if args.is_empty() => Ok(VmValue::Bool(s.is_empty())),
            "chars"    if args.is_empty() => {
                let chars: Vec<VmValue> = s.chars().map(|c| VmValue::Str(c.to_string())).collect();
                Ok(VmValue::List(Rc::new(RefCell::new(chars))))
            }
            "bytes"    if args.is_empty() => {
                let bytes: Vec<VmValue> = s.bytes().map(|b| VmValue::Int(b as i64)).collect();
                Ok(VmValue::List(Rc::new(RefCell::new(bytes))))
            }
            "lines"    if args.is_empty() => {
                let lines: Vec<VmValue> = s.lines().map(|l| VmValue::Str(l.to_string())).collect();
                Ok(VmValue::List(Rc::new(RefCell::new(lines))))
            }
            "include?" if args.len() == 1 => match &args[0] {
                VmValue::Str(pat) => Ok(VmValue::Bool(s.contains(pat.as_str()))),
                _ => Err(type_err("include? expects a String")),
            },
            "starts_with?" if args.len() == 1 => match &args[0] {
                VmValue::Str(pat) => Ok(VmValue::Bool(s.starts_with(pat.as_str()))),
                _ => Err(type_err("starts_with? expects a String")),
            },
            "ends_with?" if args.len() == 1 => match &args[0] {
                VmValue::Str(pat) => Ok(VmValue::Bool(s.ends_with(pat.as_str()))),
                _ => Err(type_err("ends_with? expects a String")),
            },
            "split" => match args {
                [] => {
                    let parts: Vec<VmValue> = s.split_whitespace().map(|p| VmValue::Str(p.to_string())).collect();
                    Ok(VmValue::List(Rc::new(RefCell::new(parts))))
                }
                [VmValue::Str(sep)] => {
                    let parts: Vec<VmValue> = s.split(sep.as_str()).map(|p| VmValue::Str(p.to_string())).collect();
                    Ok(VmValue::List(Rc::new(RefCell::new(parts))))
                }
                _ => Err(type_err("split expects a String delimiter")),
            },
            "replace" if args.len() == 2 => match (&args[0], &args[1]) {
                (VmValue::Str(from), VmValue::Str(to)) => Ok(VmValue::Str(s.replacen(from.as_str(), to.as_str(), 1))),
                _ => Err(type_err("replace expects two Strings")),
            },
            "replace_all" if args.len() == 2 => match (&args[0], &args[1]) {
                (VmValue::Str(from), VmValue::Str(to)) => Ok(VmValue::Str(s.replace(from.as_str(), to.as_str()))),
                _ => Err(type_err("replace_all expects two Strings")),
            },
            "slice" if args.len() == 2 => match (&args[0], &args[1]) {
                (VmValue::Int(start), VmValue::Int(len)) => {
                    let chars: Vec<char> = s.chars().collect();
                    let n = chars.len() as i64;
                    let start = if *start < 0 { (n + start).max(0) as usize } else { *start as usize };
                    let len = *len as usize;
                    let end = (start + len).min(chars.len());
                    Ok(VmValue::Str(chars[start..end].iter().collect()))
                }
                _ => Err(type_err("slice expects (Int, Int)")),
            },
            _ => Err(type_err(&format!("String has no method '{}'", name))),
        },

        VmValue::Bool(b) => match (name, args) {
            ("to_s",   []) => Ok(VmValue::Str(b.to_string())),
            ("nil?",   []) => Ok(VmValue::Bool(false)),
            _ => Err(type_err(&format!("Bool has no method '{}'", name))),
        },

        VmValue::Nil => match (name, args) {
            ("to_s",   []) => Ok(VmValue::Str(String::new())),
            ("nil?",   []) => Ok(VmValue::Bool(true)),
            ("inspect",[]) => Ok(VmValue::Str("nil".to_string())),
            _ => Err(type_err(&format!("Nil has no method '{}'", name))),
        },

        VmValue::List(elems) => match name {
            "size" | "length" if args.is_empty() => Ok(VmValue::Int(elems.borrow().len() as i64)),
            "empty?"   if args.is_empty() => Ok(VmValue::Bool(elems.borrow().is_empty())),
            "first"    if args.is_empty() => Ok(elems.borrow().first().cloned().unwrap_or(VmValue::Nil)),
            "last"     if args.is_empty() => Ok(elems.borrow().last().cloned().unwrap_or(VmValue::Nil)),
            "pop"      if args.is_empty() => Ok(elems.borrow_mut().pop().unwrap_or(VmValue::Nil)),
            "reverse"  if args.is_empty() => {
                let v: Vec<VmValue> = elems.borrow().iter().cloned().rev().collect();
                Ok(VmValue::List(Rc::new(RefCell::new(v))))
            }
            "sort"     if args.is_empty() => {
                let mut v: Vec<VmValue> = elems.borrow().clone();
                v.sort_by(|a, b| vm_value_partial_cmp(a, b));
                Ok(VmValue::List(Rc::new(RefCell::new(v))))
            }
            "include?" if args.len() == 1 => Ok(VmValue::Bool(elems.borrow().contains(&args[0]))),
            "push" | "append" if args.len() == 1 => {
                elems.borrow_mut().push(args[0].clone());
                Ok(recv.clone())
            }
            "unshift" | "prepend" if args.len() == 1 => {
                elems.borrow_mut().insert(0, args[0].clone());
                Ok(recv.clone())
            }
            "concat" if args.len() == 1 => match &args[0] {
                VmValue::List(other) => {
                    elems.borrow_mut().extend(other.borrow().iter().cloned());
                    Ok(recv.clone())
                }
                _ => Err(type_err("concat expects a List")),
            },
            "join" => {
                let sep = match args.first() {
                    Some(VmValue::Str(s)) => s.clone(),
                    None => String::new(),
                    _ => return Err(type_err("join expects a String")),
                };
                let s = elems.borrow().iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(&sep);
                Ok(VmValue::Str(s))
            }
            "flatten" if args.is_empty() => {
                fn flatten_list(v: &VmValue) -> Vec<VmValue> {
                    match v {
                        VmValue::List(inner) => inner.borrow().iter().flat_map(flatten_list).collect(),
                        other => vec![other.clone()],
                    }
                }
                let v = elems.borrow().iter().flat_map(flatten_list).collect();
                Ok(VmValue::List(Rc::new(RefCell::new(v))))
            }
            "uniq" if args.is_empty() => {
                let mut seen = Vec::new();
                for item in elems.borrow().iter() {
                    if !seen.contains(item) { seen.push(item.clone()); }
                }
                Ok(VmValue::List(Rc::new(RefCell::new(seen))))
            }
            "min" if args.is_empty() => {
                let v = elems.borrow();
                if v.is_empty() { return Ok(VmValue::Nil); }
                let m = v.iter().min_by(|a, b| vm_value_partial_cmp(a, b)).cloned().unwrap();
                Ok(m)
            }
            "max" if args.is_empty() => {
                let v = elems.borrow();
                if v.is_empty() { return Ok(VmValue::Nil); }
                let m = v.iter().max_by(|a, b| vm_value_partial_cmp(a, b)).cloned().unwrap();
                Ok(m)
            }
            "sum" if args.is_empty() => {
                let v = elems.borrow();
                let mut acc = VmValue::Int(0);
                for item in v.iter() {
                    acc = match (&acc, item) {
                        (VmValue::Int(a), VmValue::Int(b)) => VmValue::Int(a + b),
                        (VmValue::Float(a), VmValue::Float(b)) => VmValue::Float(a + b),
                        (VmValue::Int(a), VmValue::Float(b)) => VmValue::Float(*a as f64 + b),
                        (VmValue::Float(a), VmValue::Int(b)) => VmValue::Float(a + *b as f64),
                        _ => return Err(type_err("sum: non-numeric element")),
                    };
                }
                Ok(acc)
            }
            "any?" if args.is_empty() => Err(type_err("any? requires a block")),
            "all?" if args.is_empty() => Err(type_err("all? requires a block")),
            "to_s" if args.is_empty() => Ok(VmValue::Str(format!("{}", recv))),
            _ => Err(type_err(&format!("List has no method '{}'", name))),
        },

        VmValue::Map(map) => match name {
            "size" | "length" if args.is_empty() => Ok(VmValue::Int(map.borrow().len() as i64)),
            "empty?"   if args.is_empty() => Ok(VmValue::Bool(map.borrow().is_empty())),
            "keys"     if args.is_empty() => {
                let mut keys: Vec<VmValue> = map.borrow().keys().map(|k| VmValue::Str(k.clone())).collect();
                keys.sort_by(|a, b| vm_value_partial_cmp(a, b));
                Ok(VmValue::List(Rc::new(RefCell::new(keys))))
            }
            "values"   if args.is_empty() => {
                let mut pairs: Vec<(String, VmValue)> = map.borrow().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                let vals: Vec<VmValue> = pairs.into_iter().map(|(_, v)| v).collect();
                Ok(VmValue::List(Rc::new(RefCell::new(vals))))
            }
            "has_key?" if args.len() == 1 => match &args[0] {
                VmValue::Str(k) => Ok(VmValue::Bool(map.borrow().contains_key(k.as_str()))),
                _ => Err(type_err("has_key? expects a String")),
            },
            "get" if args.len() == 1 => match &args[0] {
                VmValue::Str(k) => Ok(map.borrow().get(k.as_str()).cloned().unwrap_or(VmValue::Nil)),
                _ => Err(type_err("get expects a String key")),
            },
            "set" if args.len() == 2 => match &args[0] {
                VmValue::Str(k) => {
                    map.borrow_mut().insert(k.clone(), args[1].clone());
                    Ok(args[1].clone())
                }
                _ => Err(type_err("set expects a String key")),
            },
            "delete" if args.len() == 1 => match &args[0] {
                VmValue::Str(k) => Ok(map.borrow_mut().remove(k.as_str()).unwrap_or(VmValue::Nil)),
                _ => Err(type_err("delete expects a String key")),
            },
            "merge" if args.len() == 1 => match &args[0] {
                VmValue::Map(other) => {
                    let mut new_map = map.borrow().clone();
                    for (k, v) in other.borrow().iter() {
                        new_map.insert(k.clone(), v.clone());
                    }
                    Ok(VmValue::Map(Rc::new(RefCell::new(new_map))))
                }
                _ => Err(type_err("merge expects a Map")),
            },
            "to_s" if args.is_empty() => Ok(VmValue::Str(format!("{}", recv))),
            _ => Err(type_err(&format!("Map has no method '{}'", name))),
        },

        VmValue::Range { from, to } => match name {
            "size" | "length" if args.is_empty() => Ok(VmValue::Int((to - from).max(0))),
            "to_a" if args.is_empty() => {
                let v: Vec<VmValue> = (*from..*to).map(VmValue::Int).collect();
                Ok(VmValue::List(Rc::new(RefCell::new(v))))
            }
            "include?" if args.len() == 1 => match &args[0] {
                VmValue::Int(n) => Ok(VmValue::Bool(n >= from && n < to)),
                _ => Err(type_err("include? expects an Int")),
            },
            "first" if args.is_empty() => Ok(VmValue::Int(*from)),
            "last"  if args.is_empty() => Ok(VmValue::Int(to - 1)),
            "min"   if args.is_empty() => Ok(VmValue::Int(*from)),
            "max"   if args.is_empty() => Ok(VmValue::Int(to - 1)),
            "to_s"  if args.is_empty() => Ok(VmValue::Str(format!("{}", recv))),
            _ => Err(type_err(&format!("Range has no method '{}'", name))),
        },

        other => Err(VmError::TypeError {
            message: format!("'{}' has no method '{}'", other, name),
            line,
        }),
    }
}

/// Like `dispatch_native_method` but returns `None` when no native handler
/// exists for this method, allowing callers to try the class registry next.
/// Any real type error (wrong arg count, wrong type, etc.) is still `Some(Err)`.
fn try_native_method(
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Option<Result<VmValue, VmError>> {
    match dispatch_native_method(recv, name, args, line) {
        Err(VmError::TypeError { ref message, .. }) if message.contains("has no method") => None,
        result => Some(result),
    }
}

/// Return the stdlib class name for a primitive value, used to look up
/// compiled stdlib methods in the class registry.
fn primitive_class_name(val: &VmValue) -> Option<&'static str> {
    match val {
        VmValue::Int(_)   => Some("Int"),
        VmValue::Float(_) => Some("Float"),
        VmValue::Str(_)   => Some("String"),
        VmValue::Bool(_)  => Some("Bool"),
        VmValue::Nil      => Some("Nil"),
        VmValue::List(_)  => Some("List"),
        VmValue::Map(_)   => Some("Map"),
        _                 => None,
    }
}

fn starting_class_name_for_is_a(recv: &VmValue) -> Option<String> {
    match recv {
        VmValue::Instance { class_name, .. } => Some(class_name.clone()),
        _ => primitive_class_name(recv).map(|s| s.to_string()),
    }
}

/// True if `start` is `target` or a subclass of `target` per `classes` superclass links.
fn vm_is_subclass(classes: &HashMap<String, ClassEntry>, mut current: String, target: &str) -> bool {
    loop {
        if current == target {
            return true;
        }
        let Some(entry) = classes.get(&current) else {
            return false;
        };
        match &entry.superclass {
            Some(s) => current = s.clone(),
            None => return false,
        }
    }
}

fn invoke_is_a(
    classes: &HashMap<String, ClassEntry>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let target = match args.first() {
        Some(VmValue::Class { name, .. }) => name.clone(),
        _ => {
            return Err(VmError::TypeError {
                message: "is_a? requires a class argument".into(),
                line,
            });
        }
    };
    let Some(start) = starting_class_name_for_is_a(recv) else {
        return Ok(VmValue::Bool(false));
    };
    Ok(VmValue::Bool(vm_is_subclass(classes, start, &target)))
}

/// Simple comparison for sorting — numbers compare numerically, strings lexicographically.
fn vm_value_partial_cmp(a: &VmValue, b: &VmValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (VmValue::Int(x),   VmValue::Int(y))   => x.cmp(y),
        (VmValue::Float(x), VmValue::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (VmValue::Int(x),   VmValue::Float(y)) => (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal),
        (VmValue::Float(x), VmValue::Int(y))   => x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal),
        (VmValue::Str(x),   VmValue::Str(y))   => x.cmp(y),
        _ => Ordering::Equal,
    }
}

fn is_falsy(v: &VmValue) -> bool {
    matches!(v, VmValue::Nil | VmValue::Bool(false))
}

fn numeric_binop(
    a: &VmValue,
    b: &VmValue,
    line: u32,
    verb: &str,
    int_op: impl Fn(i64, i64) -> i64,
    float_op: impl Fn(f64, f64) -> f64,
) -> Result<VmValue, VmError> {
    match (a, b) {
        (VmValue::Int(x),   VmValue::Int(y))   => Ok(VmValue::Int(int_op(*x, *y))),
        (VmValue::Float(x), VmValue::Float(y)) => Ok(VmValue::Float(float_op(*x, *y))),
        (VmValue::Int(x),   VmValue::Float(y)) => Ok(VmValue::Float(float_op(*x as f64, *y))),
        (VmValue::Float(x), VmValue::Int(y))   => Ok(VmValue::Float(float_op(*x, *y as f64))),
        _ => Err(VmError::TypeError {
            message: format!("cannot {} {} and {}", verb, a, b),
            line,
        }),
    }
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
