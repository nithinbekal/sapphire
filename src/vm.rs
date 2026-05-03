use crate::chunk::{Constant, Function, OpCode, RuntimeType};
use crate::gc::{GcHeap, GcRef, Trace};
use crate::native::{
    is_falsy, numeric_binop, numeric_cmp, primitive_class_name,
    value_type_name, vm_value_partial_cmp,
};
use crate::value::Value;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

fn runtime_type_display(rt: &RuntimeType) -> String {
    match rt {
        RuntimeType::Named(n) => n.clone(),
        RuntimeType::Literal(Value::Int(n)) => n.to_string(),
        RuntimeType::Literal(Value::Float(n)) => n.to_string(),
        RuntimeType::Literal(Value::Str(s)) => format!("{:?}", s),
        RuntimeType::Literal(Value::Bool(b)) => b.to_string(),
        RuntimeType::Literal(Value::Nil) => "Nil".to_string(),
        RuntimeType::Union(arms) => arms
            .iter()
            .map(runtime_type_display)
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

fn vm_value_matches_literal(value: &VmValue, literal: &Value) -> bool {
    match (value, literal) {
        (VmValue::Int(a), Value::Int(b)) => a == b,
        (VmValue::Float(a), Value::Float(b)) => a == b,
        (VmValue::Str(a), Value::Str(b)) => a == b,
        (VmValue::Bool(a), Value::Bool(b)) => a == b,
        (VmValue::Nil, Value::Nil) => true,
        _ => false,
    }
}

fn runtime_type_matches(value: &VmValue, expected: &RuntimeType) -> bool {
    match expected {
        RuntimeType::Named(e) => {
            let actual = value_type_name(value);
            actual == e || (e == "Num" && (actual == "Int" || actual == "Float"))
        }
        RuntimeType::Literal(v) => vm_value_matches_literal(value, v),
        RuntimeType::Union(arms) => arms.iter().any(|arm| runtime_type_matches(value, arm)),
    }
}

// ── GC heap objects ───────────────────────────────────────────────────────────

/// Rust function pointer for a native instance method on a `ClassObject`.
pub type NativeFn = fn(
    &mut GcHeap<HeapObject>,
    &VmValue,
    &[VmValue],
    u32,
) -> Result<VmValue, VmError>;

/// Inclusive arity bounds for a [`SapphireMethod::Native`] (`min`..=`max` arguments).
#[derive(Clone, Copy)]
pub struct NativeArity {
    pub min: usize,
    pub max: usize,
}

impl From<usize> for NativeArity {
    fn from(n: usize) -> Self {
        Self { min: n, max: n }
    }
}

impl NativeArity {
    /// Sentinel `max` meaning “no upper bound” (arity is `min` or more).
    pub const VARIADIC_MAX: usize = usize::MAX;

    pub fn at_least(min: usize) -> Self {
        Self {
            min,
            max: Self::VARIADIC_MAX,
        }
    }
}

/// A method that lives in a `ClassObject` method table.
#[derive(Clone)]
pub enum SapphireMethod {
    Bytecode(VmMethod),
    Native {
        min_arity: usize,
        max_arity: usize,
        func: NativeFn,
    },
}

/// Objects managed by the GC heap — all types that can form reference cycles.
pub enum HeapObject {
    List(Vec<VmValue>),
    Map(HashMap<String, VmValue>),
    Set(Vec<VmValue>),
    /// Instance field storage.
    Fields(HashMap<String, VmValue>),
    /// A heap-allocated class object in the Ruby-style object model.
    /// `class_ref` points to the class's own class (e.g. every ClassObject's
    /// class_ref points to the `Class` ClassObject, and `Class.class_ref`
    /// points to itself).  `None` only transiently during two-phase bootstrap.
    ClassObject {
        name: String,
        superclass: Option<GcRef>,
        class_ref: Option<GcRef>,
        methods: HashMap<String, SapphireMethod>,
        class_methods: HashMap<String, SapphireMethod>,
    },
}

impl Trace for HeapObject {
    fn trace(&self, out: &mut Vec<GcRef>) {
        match self {
            HeapObject::List(v) => v.iter().for_each(|val| collect_refs(val, out)),
            HeapObject::Map(m) => m.values().for_each(|val| collect_refs(val, out)),
            HeapObject::Set(v) => v.iter().for_each(|val| collect_refs(val, out)),
            HeapObject::Fields(f) => f.values().for_each(|val| collect_refs(val, out)),
            HeapObject::ClassObject {
                superclass,
                class_ref,
                methods,
                class_methods,
                ..
            } => {
                if let Some(r) = superclass { out.push(*r); }
                if let Some(r) = class_ref  { out.push(*r); }
                for m in methods.values().chain(class_methods.values()) {
                    if let SapphireMethod::Bytecode(vm_method) = m {
                        for uv in &vm_method.upvalues {
                            if let UpvalueState::Closed(v) = &*uv.0.borrow() {
                                collect_refs(v, out);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Push all GcRefs contained directly in `val` into `out`.
fn collect_refs(val: &VmValue, out: &mut Vec<GcRef>) {
    match val {
        VmValue::List(r) | VmValue::Map(r) | VmValue::Set(r) | VmValue::ClassObj(r) => {
            out.push(*r)
        }
        VmValue::Instance { fields, .. } => out.push(*fields),
        _ => {}
    }
}

impl GcHeap<HeapObject> {
    pub fn get_list(&self, r: GcRef) -> &Vec<VmValue> {
        match self.get(r) {
            HeapObject::List(v) => v,
            _ => panic!("GcRef is not a List"),
        }
    }
    pub fn get_list_mut(&mut self, r: GcRef) -> &mut Vec<VmValue> {
        match self.get_mut(r) {
            HeapObject::List(v) => v,
            _ => panic!("GcRef is not a List"),
        }
    }
    pub fn get_map(&self, r: GcRef) -> &HashMap<String, VmValue> {
        match self.get(r) {
            HeapObject::Map(m) => m,
            _ => panic!("GcRef is not a Map"),
        }
    }
    pub fn get_map_mut(&mut self, r: GcRef) -> &mut HashMap<String, VmValue> {
        match self.get_mut(r) {
            HeapObject::Map(m) => m,
            _ => panic!("GcRef is not a Map"),
        }
    }
    pub fn get_set(&self, r: GcRef) -> &Vec<VmValue> {
        match self.get(r) {
            HeapObject::Set(v) => v,
            _ => panic!("GcRef is not a Set"),
        }
    }
    pub fn get_set_mut(&mut self, r: GcRef) -> &mut Vec<VmValue> {
        match self.get_mut(r) {
            HeapObject::Set(v) => v,
            _ => panic!("GcRef is not a Set"),
        }
    }
    pub fn get_fields(&self, r: GcRef) -> &HashMap<String, VmValue> {
        match self.get(r) {
            HeapObject::Fields(f) => f,
            _ => panic!("GcRef is not Fields"),
        }
    }
    pub fn get_fields_mut(&mut self, r: GcRef) -> &mut HashMap<String, VmValue> {
        match self.get_mut(r) {
            HeapObject::Fields(f) => f,
            _ => panic!("GcRef is not Fields"),
        }
    }
}

/// Insert a native method into a `ClassObject`'s method table (like Ruby's `rb_define_method`).
pub fn define_native_method(
    heap: &mut GcHeap<HeapObject>,
    class_ref: GcRef,
    name: &str,
    arity: impl Into<NativeArity>,
    func: NativeFn,
) {
    let NativeArity { min, max } = arity.into();
    match heap.get_mut(class_ref) {
        HeapObject::ClassObject { methods, .. } => {
            methods.insert(
                name.to_string(),
                SapphireMethod::Native {
                    min_arity: min,
                    max_arity: max,
                    func,
                },
            );
        }
        _ => panic!("define_native_method: GcRef is not a ClassObject"),
    }
}

/// Insert a native class method into a `ClassObject`'s class-method table.
pub fn define_native_class_method(
    heap: &mut GcHeap<HeapObject>,
    class_ref: GcRef,
    name: &str,
    arity: impl Into<NativeArity>,
    func: NativeFn,
) {
    let NativeArity { min, max } = arity.into();
    match heap.get_mut(class_ref) {
        HeapObject::ClassObject { class_methods, .. } => {
            class_methods.insert(
                name.to_string(),
                SapphireMethod::Native {
                    min_arity: min,
                    max_arity: max,
                    func,
                },
            );
        }
        _ => panic!("define_native_class_method: GcRef is not a ClassObject"),
    }
}

/// Recursively format `val` using heap data for List/Map/Instance.
pub fn format_value_with_heap(heap: &GcHeap<HeapObject>, val: &VmValue) -> String {
    match val {
        VmValue::List(r) => {
            let parts: Vec<String> = heap
                .get_list(*r)
                .iter()
                .map(|el| format_value_with_heap(heap, el))
                .collect();
            format!("[{}]", parts.join(", "))
        }
        VmValue::Map(r) => {
            let mut parts: Vec<String> = heap
                .get_map(*r)
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value_with_heap(heap, v)))
                .collect();
            parts.sort();
            format!("{{{}}}", parts.join(", "))
        }
        VmValue::Set(r) => {
            let parts: Vec<String> = heap
                .get_set(*r)
                .iter()
                .map(|el| format_value_with_heap(heap, el))
                .collect();
            format!("Set{{{}}}", parts.join(", "))
        }
        VmValue::Instance {
            class_name, fields, ..
        } => {
            let mut pairs: Vec<String> = heap
                .get_fields(*fields)
                .iter()
                .map(|(k, v)| format!("{}={}", k, format_value_with_heap(heap, v)))
                .collect();
            pairs.sort();
            format!("#<{} {}>", class_name, pairs.join(", "))
        }
        other => format!("{}", other),
    }
}

// ── Upvalue ───────────────────────────────────────────────────────────────────

/// The heap-allocated cell shared between a closure and the variable it captures.
/// While the captured variable is still live on the stack the upvalue is "open"
/// (holds a stack index).  When the enclosing frame returns the upvalue is
/// "closed": the value is copied out of the stack into the cell itself.
#[derive(Debug, Clone)]
pub enum UpvalueState {
    Open(usize), // index into Vm::stack
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
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
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
    List(GcRef),
    Map(GcRef),
    Set(GcRef),
    Range {
        from: i64,
        to: i64,
    },
    /// A compiled class: holds the static field list (with defaults) and the method table.
    Class {
        name: String,
        #[allow(dead_code)]
        superclass: Option<String>,
        fields: Vec<(String, VmValue)>,
        methods: Rc<HashMap<String, VmMethod>>,
        class_methods: Rc<HashMap<String, VmMethod>>,
        /// Nested class definitions, accessible as `Outer.Inner`.
        namespace: Rc<HashMap<String, VmValue>>,
    },
    /// A live instance of a class.
    Instance {
        class_name: String,
        #[allow(dead_code)]
        ancestor_chain: Rc<Vec<String>>,
        fields: GcRef,
        methods: Rc<HashMap<String, VmMethod>>,
    },
    /// A heap-allocated class object in the Ruby-style object model.
    ClassObj(GcRef),
}

/// A compiled method: a function together with any upvalues it closed over,
/// and the name of the class that originally defined it (used by `super`).
#[derive(Debug, Clone)]
pub struct VmMethod {
    pub function: Rc<Function>,
    pub upvalues: Vec<Upvalue>,
    /// Name of the class this method was defined in; empty for block closures.
    pub defined_in: String,
    pub private: bool,
}

impl PartialEq for VmValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VmValue::Int(a), VmValue::Int(b)) => a == b,
            (VmValue::Float(a), VmValue::Float(b)) => a == b,
            (VmValue::Str(a), VmValue::Str(b)) => a == b,
            (VmValue::Bool(a), VmValue::Bool(b)) => a == b,
            (VmValue::Nil, VmValue::Nil) => true,
            (VmValue::Function(a), VmValue::Function(b)) => Rc::ptr_eq(a, b),
            (VmValue::Closure { function: f1, .. }, VmValue::Closure { function: f2, .. }) => {
                Rc::ptr_eq(f1, f2)
            }
            (VmValue::List(a), VmValue::List(b)) => a == b,
            (VmValue::Map(a), VmValue::Map(b)) => a == b,
            (VmValue::Set(a), VmValue::Set(b)) => a == b,
            (VmValue::Range { from: f1, to: t1 }, VmValue::Range { from: f2, to: t2 }) => {
                f1 == f2 && t1 == t2
            }
            (VmValue::Instance { fields: f1, .. }, VmValue::Instance { fields: f2, .. }) => {
                f1 == f2
            }
            (VmValue::ClassObj(a), VmValue::ClassObj(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmValue::Int(n) => write!(f, "{}", n),
            VmValue::Float(n) => write!(f, "{}", n),
            VmValue::Str(s) => write!(f, "{}", s),
            VmValue::Bool(b) => write!(f, "{}", b),
            VmValue::Nil => write!(f, "nil"),
            VmValue::Function(func) => write!(f, "<fn {}>", func.name),
            VmValue::Closure { function, .. } => write!(f, "<fn {}>", function.name),
            VmValue::List(_) => write!(f, "<list>"),
            VmValue::Map(_) => write!(f, "<map>"),
            VmValue::Set(_) => write!(f, "<set>"),
            VmValue::Range { from, to } => write!(f, "{}..{}", from, to),
            VmValue::Class { name, .. } => write!(f, "<class {}>", name),
            VmValue::Instance { class_name, .. } => write!(f, "#<{}>", class_name),
            VmValue::ClassObj(_) => write!(f, "<class>"),
        }
    }
}

impl From<&Constant> for VmValue {
    fn from(c: &Constant) -> Self {
        match c {
            Constant::Int(n) => VmValue::Int(*n),
            Constant::Float(n) => VmValue::Float(*n),
            Constant::Str(s) => VmValue::Str(s.clone()),
            Constant::Function(func) => VmValue::Function(func.clone()),
            Constant::ClassDesc { .. } => panic!("ClassDesc cannot be used as a stack value"),
            Constant::LexicalClassScope { .. } => {
                panic!("LexicalClassScope cannot be used as a stack value")
            }
        }
    }
}

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum VmError {
    StackUnderflow,
    TypeError {
        message: String,
        line: u32,
    },
    /// `raise val` — propagates until caught by a `Begin` handler.
    Raised(VmValue),
    /// `break val` inside a block — unwinds to the enclosing call-with-block.
    Break(VmValue),
    /// `next val` inside a block — skips to the next `yield`.
    #[allow(dead_code)]
    Next(VmValue),
    /// `return val` inside a block called by a native method — propagates to
    /// the dispatch site so it can perform a non-local return from the
    /// enclosing Sapphire frame.
    Return(Option<VmValue>),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::StackUnderflow => {
                write!(f, "internal error: stack underflow (this is a Sapphire bug)")
            }
            VmError::TypeError { message, line } => {
                write!(f, "[line {}] error: {}", line, message)
            }
            VmError::Raised(v) => write!(f, "uncaught raise: {}", v),
            VmError::Break(v) => write!(f, "break outside block: {}", v),
            VmError::Next(v) => write!(f, "next outside block: {}", v),
            VmError::Return(v) => write!(f, "return outside method: {:?}", v),
        }
    }
}

// ── Call frame ────────────────────────────────────────────────────────────────

/// Rescue handler registered by `BeginRescue`; popped by `PopRescue`.
#[derive(Clone, Copy)]
struct RescueInfo {
    handler_ip: usize,
    rescue_var_slot: usize, // usize::MAX means no variable
    stack_height: usize,    // stack depth at BeginRescue time (for cleanup)
}

struct CallFrame {
    function: Rc<Function>,
    /// The upvalues belonging to the closure that created this frame.
    upvalues: Vec<Upvalue>,
    /// Instruction pointer within this frame's chunk.
    ip: usize,
    /// Index into the VM stack where slot 0 of this frame begins.
    /// The function value itself lives at `base`.
    base: usize,
    /// Block passed to this call (if any), stored off-stack so `yield` can
    /// reach it without disturbing the locals layout.
    block: Option<VmMethod>,
    /// True if this frame was pushed by `CallWithBlock`/`InvokeWithBlock`.
    /// Signals `break` to stop unwinding here.
    is_block_caller: bool,
    /// True if this frame was pushed by `call_block` on behalf of a native
    /// method.  Signals `break` to stop unwinding and propagate as an error
    /// so the native dispatch can catch and handle it.
    is_native_block: bool,
    /// Active rescue handlers within this frame (push on BeginRescue, pop on PopRescue).
    rescues: Vec<RescueInfo>,
    /// The class that defines the method running in this frame; None for
    /// non-method frames (plain functions, blocks).  Used by `SuperInvoke`.
    class_name: Option<String>,
}

// ── VM ────────────────────────────────────────────────────────────────────────

/// GcRefs to the core class objects bootstrapped before stdlib loads.
/// Kept here so they are always reachable as GC roots.
#[derive(Debug, Clone, Default)]
pub struct CoreClasses {
    pub object: Option<GcRef>,
    pub class_cls: Option<GcRef>,
    pub set_cls: Option<GcRef>,
    pub nil_cls: Option<GcRef>,
    pub int_cls: Option<GcRef>,
    pub float_cls: Option<GcRef>,
    pub string_cls: Option<GcRef>,
    pub range_cls: Option<GcRef>,
    pub list_cls: Option<GcRef>,
    pub map_cls: Option<GcRef>,
    pub env_cls: Option<GcRef>,
    pub file_cls: Option<GcRef>,
}

/// Per-class metadata stored by DefClass.
///
/// `methods` holds the *merged* (inherited + own) method table — the same map
/// that lives on `VmValue::Class`.  Using the merged table means:
///
/// * Primitive dispatch (`Invoke` on Int/Str/…) finds inherited Object methods.
/// * `SuperInvoke` looks up `classes[super_name].methods` and gets the
///   correct merged map for that ancestor level.
struct ClassEntry {
    superclass: Option<String>,
    /// Linearized ancestors (class/module name chain) for `super` and `is_a?`.
    ancestors: Vec<String>,
    /// Included mixin names in source order (registry keys).
    includes: Vec<String>,
    is_module: bool,
    /// The merged (inherited + own) field list with default values.
    fields: Vec<(String, VmValue)>,
    /// Merged (inherited + own) instance methods.
    methods: Rc<HashMap<String, VmMethod>>,
    /// Merged (inherited + own) class methods.
    class_methods: Rc<HashMap<String, VmMethod>>,
    /// Constants defined in the class body (e.g. `PI = 3.14`).
    namespace: Rc<HashMap<String, VmValue>>,
}

pub struct Vm {
    frames: Vec<CallFrame>,
    stack: Vec<VmValue>,
    /// GC-managed heap for List, Map, and instance field objects.
    pub heap: GcHeap<HeapObject>,
    /// All upvalues that still point into the live stack (open upvalues),
    /// kept so we can close them when a frame exits.
    open_upvalues: Vec<Upvalue>,
    /// Registry of every class defined so far, keyed by name.  Stores each
    /// class's own (non-merged) methods so SuperInvoke can dispatch to them.
    classes: HashMap<String, ClassEntry>,
    /// Global variable store used by the REPL to persist values across calls.
    pub globals: HashMap<String, VmValue>,
    /// Directory of the currently-executing top-level file; used to resolve
    /// relative import paths.  Empty for the REPL (imports not supported).
    current_dir: PathBuf,
    /// Canonicalized paths of files already imported; prevents double-loading.
    imported: HashSet<PathBuf>,
    /// When `Some`, print output is buffered here instead of written to stdout.
    pub output: Option<Vec<String>>,
    /// Open TCP sockets keyed by integer fd; lives outside the GC.
    sockets: HashMap<i64, std::io::BufReader<std::net::TcpStream>>,
    next_socket_id: i64,
    /// Compiled regexes keyed by integer id; lives outside the GC.
    regexes: HashMap<i64, regex::Regex>,
    next_regex_id: i64,
    /// GcRefs to the bootstrapped core class objects (Object, Class, Set, …).
    core_classes: CoreClasses,
}

impl Vm {
    pub fn new(function: Rc<Function>, current_dir: PathBuf) -> Self {
        let frame = CallFrame {
            function,
            upvalues: vec![],
            ip: 0,
            base: 0,
            block: None,
            is_block_caller: false,
            is_native_block: false,
            rescues: vec![],
            class_name: None,
        };
        let mut vm = Vm {
            frames: vec![frame],
            stack: Vec::new(),
            heap: GcHeap::new(),
            open_upvalues: Vec::new(),
            classes: HashMap::new(),
            globals: HashMap::new(),
            current_dir,
            imported: HashSet::new(),
            output: None,
            sockets: HashMap::new(),
            next_socket_id: 0,
            regexes: HashMap::new(),
            next_regex_id: 0,
            core_classes: CoreClasses::default(),
        };
        vm.bootstrap_core_classes();
        vm
    }

    /// Create an empty VM with no initial frame, for use in the REPL.
    /// Call `load_stdlib()` before evaluating any user code.
    pub fn new_repl() -> Self {
        let mut vm = Vm {
            frames: vec![],
            stack: Vec::new(),
            heap: GcHeap::new(),
            open_upvalues: Vec::new(),
            classes: HashMap::new(),
            globals: HashMap::new(),
            current_dir: PathBuf::new(),
            imported: HashSet::new(),
            output: None,
            sockets: HashMap::new(),
            next_socket_id: 0,
            regexes: HashMap::new(),
            next_regex_id: 0,
            core_classes: CoreClasses::default(),
        };
        vm.bootstrap_core_classes();
        vm
    }

    /// Evaluate a compiled function in the current VM context and return its result.
    /// Used by the REPL to run each input snippet while preserving global state.
    pub fn eval(&mut self, func: Rc<Function>) -> Result<Option<VmValue>, VmError> {
        let min_depth = self.frames.len();
        let base = self.stack.len();
        self.stack.push(VmValue::Function(func.clone()));
        self.frames.push(CallFrame {
            function: func,
            upvalues: vec![],
            ip: 0,
            base,
            block: None,
            is_block_caller: false,
            is_native_block: false,
            rescues: vec![],
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
            function: func,
            upvalues: vec![],
            ip: 0,
            base,
            block: None,
            is_block_caller: false,
            is_native_block: false,
            rescues: vec![],
            class_name: None,
        });
        self.run_inner(min_depth)?;
        self.stack.truncate(base);
        Ok(())
    }

    // ── Heap helpers ──────────────────────────────────────────────────────────

    fn get_list(&self, r: GcRef) -> &Vec<VmValue> {
        self.heap.get_list(r)
    }
    fn get_list_mut(&mut self, r: GcRef) -> &mut Vec<VmValue> {
        self.heap.get_list_mut(r)
    }
    fn get_map(&self, r: GcRef) -> &HashMap<String, VmValue> {
        self.heap.get_map(r)
    }
    fn get_map_mut(&mut self, r: GcRef) -> &mut HashMap<String, VmValue> {
        self.heap.get_map_mut(r)
    }
    fn get_set(&self, r: GcRef) -> &Vec<VmValue> {
        self.heap.get_set(r)
    }
    fn get_fields(&self, r: GcRef) -> &HashMap<String, VmValue> {
        self.heap.get_fields(r)
    }
    fn get_fields_mut(&mut self, r: GcRef) -> &mut HashMap<String, VmValue> {
        self.heap.get_fields_mut(r)
    }

    fn alloc_list(&mut self, v: Vec<VmValue>) -> VmValue {
        self.maybe_gc();
        VmValue::List(self.heap.alloc(HeapObject::List(v)))
    }
    fn alloc_map(&mut self, m: HashMap<String, VmValue>) -> VmValue {
        self.maybe_gc();
        VmValue::Map(self.heap.alloc(HeapObject::Map(m)))
    }
    fn alloc_set(&mut self, v: Vec<VmValue>) -> VmValue {
        self.maybe_gc();
        VmValue::Set(self.heap.alloc(HeapObject::Set(v)))
    }
    fn alloc_fields(&mut self, m: HashMap<String, VmValue>) -> GcRef {
        self.maybe_gc();
        self.heap.alloc(HeapObject::Fields(m))
    }

    /// DFS expansion of a module and its transitive `include`s (dependencies first), deduped.
    fn mixin_expansion_order(
        &self,
        module_name: &str,
        visiting: &mut HashSet<String>,
        line: u32,
    ) -> Result<Vec<String>, VmError> {
        if visiting.contains(module_name) {
            return Err(VmError::TypeError {
                message: format!("cyclic module include involving '{}'", module_name),
                line,
            });
        }
        let entry = self.classes.get(module_name).ok_or_else(|| VmError::TypeError {
            message: format!("module '{}' not found", module_name),
            line,
        })?;
        if !entry.is_module {
            return Err(VmError::TypeError {
                message: format!("include expects a module, '{}' is a class", module_name),
                line,
            });
        }
        visiting.insert(module_name.to_string());
        let mut seq: Vec<String> = Vec::new();
        for inc in &entry.includes {
            let part = self.mixin_expansion_order(inc, visiting, line)?;
            for m in part {
                if !seq.contains(&m) {
                    seq.push(m);
                }
            }
        }
        seq.push(module_name.to_string());
        visiting.remove(module_name);
        Ok(seq)
    }

    /// Ancestor chain for `super` / `is_a?`: mixin expansion (classes only), then this type, then superclass chain.
    fn ancestor_list_for_defines(
        &self,
        name: &str,
        effective_super: Option<&str>,
        includes: &[String],
        is_module: bool,
        line: u32,
    ) -> Result<Vec<String>, VmError> {
        let mut chain = vec![name.to_string()];
        if !is_module {
            // Later `include` wins for method lookup; it sits closer to the class in ancestors.
            for inc in includes.iter().rev() {
                let exp = self.mixin_expansion_order(inc, &mut HashSet::new(), line)?;
                for m in exp {
                    if !chain.contains(&m) {
                        chain.push(m);
                    }
                }
            }
        }
        if let Some(sname) = effective_super {
            let parent_chain = self.classes.get(sname).map(|e| e.ancestors.clone()).ok_or_else(
                || VmError::TypeError {
                    message: format!("superclass '{}' not found", sname),
                    line,
                },
            )?;
            for a in parent_chain {
                if !chain.contains(&a) {
                    chain.push(a);
                }
            }
        }
        Ok(chain)
    }

    /// Materialise a `DtValue` returned by the datetime dispatch module into a
    /// fully-formed `VmValue`.  For `NewInstance` we look up the class entry in
    /// the registry to obtain the compiled method table.
    fn finalize_dt(
        &mut self,
        dt: crate::datetime::DtValue,
        line: u32,
    ) -> Result<VmValue, VmError> {
        match dt {
            crate::datetime::DtValue::Value(v) => Ok(v),
            crate::datetime::DtValue::NewInstance { class_name, fields } => {
                let methods = self
                    .classes
                    .get(&class_name)
                    .map(|e| e.methods.clone())
                    .ok_or_else(|| VmError::TypeError {
                        message: format!(
                            "datetime class '{}' not loaded; \
                             call vm.load_stdlib() first",
                            class_name
                        ),
                        line,
                    })?;
                let ancestor_chain = Rc::new(
                    self.classes
                        .get(&class_name)
                        .map(|e| e.ancestors.clone())
                        .unwrap_or_else(|| vec![class_name.clone()]),
                );
                let gc_fields = self.alloc_fields(fields);
                Ok(VmValue::Instance {
                    class_name,
                    ancestor_chain,
                    fields: gc_fields,
                    methods,
                })
            }
        }
    }

    fn dispatch_socket_class(
        &mut self,
        method_name: &str,
        args: &[VmValue],
        line: u32,
    ) -> Result<VmValue, VmError> {
        match method_name {
            "connect" => {
                let (host, port) = match args {
                    [VmValue::Str(h), VmValue::Int(p)] => (h.clone(), *p),
                    _ => {
                        return Err(VmError::TypeError {
                            message: "Socket.connect expects (String, Int)".into(),
                            line,
                        })
                    }
                };
                let reader = crate::native_socket::socket_connect(&host, port, line)?;
                let id = self.next_socket_id;
                self.next_socket_id += 1;
                self.sockets.insert(id, reader);
                let methods = self
                    .classes
                    .get("Socket")
                    .map(|e| e.methods.clone())
                    .ok_or_else(|| VmError::TypeError {
                        message: "Socket class not loaded; call load_stdlib() first".into(),
                        line,
                    })?;
                let mut fields_map = HashMap::new();
                fields_map.insert("fd".to_string(), VmValue::Int(id));
                let fields_ref = self.alloc_fields(fields_map);
                Ok(VmValue::Instance {
                    class_name: "Socket".to_string(),
                    ancestor_chain: Rc::new(
                        self.classes
                            .get("Socket")
                            .map(|e| e.ancestors.clone())
                            .unwrap_or_else(|| vec!["Socket".to_string()]),
                    ),
                    fields: fields_ref,
                    methods,
                })
            }
            _ => Err(VmError::TypeError {
                message: format!("Socket has no class method '{}'", method_name),
                line,
            }),
        }
    }

    fn dispatch_socket_instance(
        &mut self,
        fields_ref: crate::gc::GcRef,
        method_name: &str,
        args: &[VmValue],
        line: u32,
    ) -> Result<VmValue, VmError> {
        let fields = self.heap.get_fields(fields_ref).clone();
        let fd = crate::native_socket::extract_fd(&fields, line)?;
        let closed_err = || VmError::Raised(VmValue::Str(format!("socket fd {} is closed", fd)));
        match method_name {
            "write" => {
                let data = match args {
                    [VmValue::Str(s)] => s.clone(),
                    _ => {
                        return Err(VmError::TypeError {
                            message: "socket.write expects a String".into(),
                            line,
                        })
                    }
                };
                let reader = self.sockets.get_mut(&fd).ok_or_else(closed_err)?;
                crate::native_socket::socket_write(reader, &data, line)?;
                Ok(VmValue::Nil)
            }
            "read_line" => {
                if !args.is_empty() {
                    return Err(VmError::TypeError {
                        message: "socket.read_line takes no arguments".into(),
                        line,
                    });
                }
                let reader = self.sockets.get_mut(&fd).ok_or_else(closed_err)?;
                crate::native_socket::socket_read_line(reader, line).map(VmValue::Str)
            }
            "read_bytes" => {
                let n = match args {
                    [VmValue::Int(n)] => *n,
                    _ => {
                        return Err(VmError::TypeError {
                            message: "socket.read_bytes expects an Int".into(),
                            line,
                        })
                    }
                };
                let reader = self.sockets.get_mut(&fd).ok_or_else(closed_err)?;
                crate::native_socket::socket_read_bytes(reader, n, line).map(VmValue::Str)
            }
            "read_all" => {
                if !args.is_empty() {
                    return Err(VmError::TypeError {
                        message: "socket.read_all takes no arguments".into(),
                        line,
                    });
                }
                let reader = self.sockets.get_mut(&fd).ok_or_else(closed_err)?;
                crate::native_socket::socket_read_all(reader, line).map(VmValue::Str)
            }
            "close" => {
                self.sockets.remove(&fd);
                Ok(VmValue::Nil)
            }
            _ => Err(VmError::TypeError {
                message: format!("Socket has no method '{}'", method_name),
                line,
            }),
        }
    }

    fn dispatch_regex_instance(
        &mut self,
        fields_ref: crate::gc::GcRef,
        method_name: &str,
        args: &[VmValue],
        line: u32,
    ) -> Result<VmValue, VmError> {
        let fields = self.heap.get_fields(fields_ref).clone();
        let id = crate::native_regex::extract_id(&fields, line)?;
        let re = self.regexes.get(&id).ok_or_else(|| {
            VmError::TypeError {
                message: format!("regex id {} not found", id),
                line,
            }
        })?;
        match method_name {
            "match?" => {
                let text = match args {
                    [VmValue::Str(s)] => s.clone(),
                    _ => {
                        return Err(VmError::TypeError {
                            message: "regex.match? expects a String".into(),
                            line,
                        })
                    }
                };
                Ok(VmValue::Bool(crate::native_regex::regex_match_bool(re, &text)))
            }
            "match" => {
                let text = match args {
                    [VmValue::Str(s)] => s.clone(),
                    _ => {
                        return Err(VmError::TypeError {
                            message: "regex.match expects a String".into(),
                            line,
                        })
                    }
                };
                match re.captures(&text) {
                    None => Ok(VmValue::Nil),
                    Some(caps) => {
                        let full = caps.get(0).unwrap().as_str().to_string();
                        let start = caps.get(0).unwrap().start() as i64;
                        let end = caps.get(0).unwrap().end() as i64;
                        let capture_list: Vec<VmValue> = caps
                            .iter()
                            .skip(1)
                            .map(|m| match m {
                                Some(m) => VmValue::Str(m.as_str().to_string()),
                                None => VmValue::Nil,
                            })
                            .collect();
                        let methods = self
                            .classes
                            .get("Match")
                            .map(|e| e.methods.clone())
                            .ok_or_else(|| VmError::TypeError {
                                message: "Regex.Match class not loaded".to_string(),
                                line,
                            })?;
                        let mut match_fields = HashMap::new();
                        match_fields.insert("full".to_string(), VmValue::Str(full));
                        match_fields.insert("captures".to_string(), self.alloc_list(capture_list));
                        match_fields.insert("start".to_string(), VmValue::Int(start));
                        match_fields.insert("end_pos".to_string(), VmValue::Int(end));
                        let gc_fields = self.alloc_fields(match_fields);
                        Ok(VmValue::Instance {
                            class_name: "Match".to_string(),
                            ancestor_chain: Rc::new(
                                self.classes
                                    .get("Match")
                                    .map(|e| e.ancestors.clone())
                                    .unwrap_or_else(|| vec!["Match".to_string()]),
                            ),
                            fields: gc_fields,
                            methods,
                        })
                    }
                }
            }
            "scan" => {
                let text = match args {
                    [VmValue::Str(s)] => s.clone(),
                    _ => {
                        return Err(VmError::TypeError {
                            message: "regex.scan expects a String".into(),
                            line,
                        })
                    }
                };
                let matches = crate::native_regex::regex_scan(re, &text);
                let match_vals: Vec<VmValue> = matches.into_iter().map(VmValue::Str).collect();
                Ok(self.alloc_list(match_vals))
            }
            "replace" => {
                let (text, replacement) = match args {
                    [VmValue::Str(t), VmValue::Str(r)] => (t.clone(), r.clone()),
                    _ => {
                        return Err(VmError::TypeError {
                            message: "regex.replace expects (String, String)".into(),
                            line,
                        })
                    }
                };
                Ok(VmValue::Str(crate::native_regex::regex_replace(
                    re,
                    &text,
                    &replacement,
                )))
            }
            "replace_all" => {
                let (text, replacement) = match args {
                    [VmValue::Str(t), VmValue::Str(r)] => (t.clone(), r.clone()),
                    _ => {
                        return Err(VmError::TypeError {
                            message: "regex.replace_all expects (String, String)".into(),
                            line,
                        })
                    }
                };
                Ok(VmValue::Str(crate::native_regex::regex_replace_all(
                    re,
                    &text,
                    &replacement,
                )))
            }
            _ => Err(VmError::TypeError {
                message: format!("Regex has no method '{}'", method_name),
                line,
            }),
        }
    }

    pub fn format_value(&self, val: &VmValue) -> String {
        format_value_with_heap(&self.heap, val)
    }

    fn gc_roots(&self) -> Vec<GcRef> {
        let mut out = Vec::new();
        for v in &self.stack {
            collect_refs(v, &mut out);
        }
        for v in self.globals.values() {
            collect_refs(v, &mut out);
        }
        for frame in &self.frames {
            for uv in &frame.upvalues {
                if let UpvalueState::Closed(v) = &*uv.0.borrow() {
                    collect_refs(v, &mut out);
                }
            }
        }
        // Class field defaults may also contain GcRefs (e.g. default list values).
        for entry in self.classes.values() {
            for (_, v) in &entry.fields {
                collect_refs(v, &mut out);
            }
        }
        // Core class objects are permanent roots.
        for r in [
            self.core_classes.object,
            self.core_classes.class_cls,
            self.core_classes.set_cls,
            self.core_classes.nil_cls,
            self.core_classes.int_cls,
            self.core_classes.float_cls,
            self.core_classes.string_cls,
            self.core_classes.range_cls,
            self.core_classes.list_cls,
            self.core_classes.map_cls,
            self.core_classes.env_cls,
            self.core_classes.file_cls,
        ]
        .into_iter()
        .flatten()
        {
            out.push(r);
        }
        out
    }

    fn maybe_gc(&mut self) {
        if self.heap.should_collect() {
            let roots = self.gc_roots();
            self.heap.collect(&roots);
        }
    }

    /// Allocate the Object / Class / core primitive `ClassObject`s and wire `class_ref`.
    /// Called from `Vm::new` / `new_repl` so `Invoke` works before `load_stdlib`, and
    /// so `DefClass` during stdlib load can mirror bytecode into these objects.
    fn bootstrap_core_classes(&mut self) {
        let object = self.heap.alloc(HeapObject::ClassObject {
            name: "Object".into(),
            superclass: None,
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let class_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "Class".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let set_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "Set".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let nil_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "Nil".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let int_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "Int".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let float_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "Float".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let string_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "String".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let range_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "Range".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let list_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "List".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let map_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "Map".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let env_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "Env".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });
        let file_cls = self.heap.alloc(HeapObject::ClassObject {
            name: "File".into(),
            superclass: Some(object),
            class_ref: None,
            methods: HashMap::new(),
            class_methods: HashMap::new(),
        });

        // Two-phase fixup: set class_ref now that class_cls is known.
        for r in [
            object,
            class_cls,
            set_cls,
            nil_cls,
            int_cls,
            float_cls,
            string_cls,
            range_cls,
            list_cls,
            map_cls,
            env_cls,
            file_cls,
        ] {
            if let HeapObject::ClassObject { class_ref, .. } = self.heap.get_mut(r) {
                *class_ref = Some(class_cls);
            }
        }

        self.core_classes = CoreClasses {
            object: Some(object),
            class_cls: Some(class_cls),
            set_cls: Some(set_cls),
            nil_cls: Some(nil_cls),
            int_cls: Some(int_cls),
            float_cls: Some(float_cls),
            string_cls: Some(string_cls),
            range_cls: Some(range_cls),
            list_cls: Some(list_cls),
            map_cls: Some(map_cls),
            env_cls: Some(env_cls),
            file_cls: Some(file_cls),
        };
        crate::native_set::register_methods(&mut self.heap, set_cls);
        crate::native_env::register_class_methods(&mut self.heap, env_cls);
        crate::native_file::register_class_methods(&mut self.heap, file_cls);
        crate::native_nil::register_methods(&mut self.heap, nil_cls);
        crate::native_int::register_methods(&mut self.heap, int_cls);
        crate::native_float::register_methods(&mut self.heap, float_cls);
        crate::native_string::register_methods(&mut self.heap, string_cls);
        crate::native_range::register_methods(&mut self.heap, range_cls);
        crate::native_list::register_methods(&mut self.heap, list_cls);
        crate::native_map::register_methods(&mut self.heap, map_cls);
    }

    /// Bootstrapped `ClassObject` for this primitive receiver, if any.
    fn class_object_for_primitive(&self, recv: &VmValue) -> Option<GcRef> {
        match recv {
            VmValue::Float(_) => self.core_classes.float_cls,
            VmValue::Int(_) => self.core_classes.int_cls,
            VmValue::Nil => self.core_classes.nil_cls,
            VmValue::Set(_) => self.core_classes.set_cls,
            VmValue::Str(_) => self.core_classes.string_cls,
            VmValue::Range { .. } => self.core_classes.range_cls,
            VmValue::List(_) => self.core_classes.list_cls,
            VmValue::Map(_) => self.core_classes.map_cls,
            _ => None,
        }
    }

    /// Walk the `ClassObject` superclass chain from `start` and return the
    /// first method named `name`, if any.
    fn lookup_class_object_method(&self, mut start: GcRef, name: &str) -> Option<SapphireMethod> {
        loop {
            let superclass = match self.heap.get(start) {
                HeapObject::ClassObject { methods, superclass, .. } => {
                    if let Some(m) = methods.get(name) {
                        return Some(m.clone());
                    }
                    *superclass
                }
                _ => return None,
            };
            start = superclass?;
        }
    }

    /// Walk the `ClassObject` superclass chain and return the first **class method**
    /// named `name`, if any.
    fn lookup_class_object_class_method(
        &self,
        mut start: GcRef,
        name: &str,
    ) -> Option<SapphireMethod> {
        loop {
            let superclass = match self.heap.get(start) {
                HeapObject::ClassObject {
                    class_methods,
                    superclass,
                    ..
                } => {
                    if let Some(m) = class_methods.get(name) {
                        return Some(m.clone());
                    }
                    *superclass
                }
                _ => return None,
            };
            start = superclass?;
        }
    }

    /// Return the `GcRef` for the bootstrapped ClassObject with the given name,
    /// if one exists.
    fn find_core_class_obj(&self, name: &str) -> Option<GcRef> {
        match name {
            "Object" => self.core_classes.object,
            "Class"  => self.core_classes.class_cls,
            "Set"    => self.core_classes.set_cls,
            "Nil"    => self.core_classes.nil_cls,
            "Int"    => self.core_classes.int_cls,
            "Float"  => self.core_classes.float_cls,
            "String" => self.core_classes.string_cls,
            "Range" => self.core_classes.range_cls,
            "List" => self.core_classes.list_cls,
            "Map" => self.core_classes.map_cls,
            "Env" => self.core_classes.env_cls,
            "File" => self.core_classes.file_cls,
            _ => None,
        }
    }

    /// Compile and execute the stdlib Sapphire files to populate the class registry.
    pub fn load_stdlib(&mut self) -> Result<(), VmError> {
        const SOURCES: &[(&str, &str)] = &[
            ("stdlib/object.spr", include_str!("../stdlib/src/object.spr")),
            ("stdlib/nil.spr", include_str!("../stdlib/src/nil.spr")),
            ("stdlib/num.spr", include_str!("../stdlib/src/num.spr")),
            ("stdlib/int.spr", include_str!("../stdlib/src/int.spr")),
            ("stdlib/float.spr", include_str!("../stdlib/src/float.spr")),
            ("stdlib/string.spr", include_str!("../stdlib/src/string.spr")),
            ("stdlib/bool.spr", include_str!("../stdlib/src/bool.spr")),
            ("stdlib/list.spr", include_str!("../stdlib/src/list.spr")),
            ("stdlib/map.spr", include_str!("../stdlib/src/map.spr")),
            ("stdlib/set.spr", include_str!("../stdlib/src/set.spr")),
            ("stdlib/regex.spr", include_str!("../stdlib/src/regex.spr")),
            ("stdlib/test.spr", include_str!("../stdlib/src/test.spr")),
            ("stdlib/file.spr", include_str!("../stdlib/src/file.spr")),
            ("stdlib/env.spr", include_str!("../stdlib/src/env.spr")),
            ("stdlib/process.spr", include_str!("../stdlib/src/process.spr")),
            ("stdlib/math.spr", include_str!("../stdlib/src/math.spr")),
            ("stdlib/duration.spr", include_str!("../stdlib/src/duration.spr")),
            ("stdlib/instant.spr", include_str!("../stdlib/src/instant.spr")),
            ("stdlib/date.spr", include_str!("../stdlib/src/date.spr")),
            ("stdlib/time.spr", include_str!("../stdlib/src/time.spr")),
            ("stdlib/datetime.spr", include_str!("../stdlib/src/datetime.spr")),
            ("stdlib/zoned_date_time.spr", include_str!("../stdlib/src/zoned_date_time.spr")),
            ("stdlib/socket.spr", include_str!("../stdlib/src/socket.spr")),
        ];
        for (name, src) in SOURCES {
            let tokens = crate::lexer::Lexer::new(src).scan_tokens();
            let stmts =
                crate::parser::Parser::new(tokens)
                    .parse()
                    .map_err(|e| VmError::TypeError {
                        message: format!("{}: {}", name, e),
                        line: 0,
                    })?;
            let func = crate::compiler::compile(&stmts).map_err(|e| VmError::TypeError {
                message: format!("{}: {}", name, e),
                line: 0,
            })?;
            self.run_extra(func)?;
        }
        // Expose all stdlib classes as globals so user code can reference them
        // by name (e.g. `File.read(...)`, `class Foo < Test`).
        let class_names: Vec<String> = self.classes.keys().cloned().collect();
        for cname in class_names {
            let entry = &self.classes[&cname];
            let val = VmValue::Class {
                name: cname.clone(),
                superclass: entry.superclass.clone(),
                fields: entry.fields.clone(),
                methods: entry.methods.clone(),
                class_methods: entry.class_methods.clone(),
                namespace: entry.namespace.clone(),
            };
            self.globals.insert(cname, val);
        }
        // Overwrite bootstrapped class names with their ClassObj values so that
        // `Object`, `Class`, and `Set` resolve to the new heap-allocated class
        // objects rather than old-style VmValue::Class entries.
        let cc = self.core_classes.clone();
        if let Some(r) = cc.object    { self.globals.insert("Object".into(), VmValue::ClassObj(r)); }
        if let Some(r) = cc.class_cls { self.globals.insert("Class".into(),  VmValue::ClassObj(r)); }
        if let Some(r) = cc.set_cls   { self.globals.insert("Set".into(),    VmValue::ClassObj(r)); }
        if let Some(r) = cc.nil_cls   { self.globals.insert("Nil".into(),    VmValue::ClassObj(r)); }
        if let Some(r) = cc.int_cls  { self.globals.insert("Int".into(),    VmValue::ClassObj(r)); }
        if let Some(r) = cc.float_cls { self.globals.insert("Float".into(), VmValue::ClassObj(r)); }
        if let Some(r) = cc.string_cls {
            self.globals.insert("String".into(), VmValue::ClassObj(r));
        }
        if let Some(r) = cc.range_cls {
            self.globals.insert("Range".into(), VmValue::ClassObj(r));
        }
        if let Some(r) = cc.list_cls {
            self.globals.insert("List".into(), VmValue::ClassObj(r));
        }
        if let Some(r) = cc.map_cls {
            self.globals.insert("Map".into(), VmValue::ClassObj(r));
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
                let op = frame.function.chunk.code[frame.ip].clone();
                let line = frame.function.chunk.lines[frame.ip];
                frame.ip += 1;
                (op, line)
            };

            match op {
                OpCode::Constant(idx) => {
                    let val =
                        VmValue::from(&self.frames.last().unwrap().function.chunk.constants[idx]);
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
                    let defs: Vec<(bool, usize)> = func
                        .upvalue_defs
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
                    self.stack.push(VmValue::Closure {
                        function: func,
                        upvalues,
                    });
                }

                OpCode::True => self.stack.push(VmValue::Bool(true)),
                OpCode::False => self.stack.push(VmValue::Bool(false)),
                OpCode::Nil => self.stack.push(VmValue::Nil),

                OpCode::Pop => {
                    self.pop()?;
                }

                // Local slots are relative to the current frame's base.
                OpCode::GetLocal(slot) => {
                    let base = self.frames.last().unwrap().base;
                    let val = self.stack[base + slot].clone();
                    self.stack.push(val);
                }
                OpCode::SetLocal(slot) => {
                    let base = self.frames.last().unwrap().base;
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?.clone();
                    self.stack[base + slot] = val;
                }

                OpCode::GetUpvalue(idx) => {
                    let uv = self.frames.last().unwrap().upvalues[idx].clone();
                    let val = match &*uv.0.borrow() {
                        UpvalueState::Open(stack_idx) => self.stack[*stack_idx].clone(),
                        UpvalueState::Closed(val) => val.clone(),
                    };
                    self.stack.push(val);
                }
                OpCode::SetUpvalue(idx) => {
                    let uv = self.frames.last().unwrap().upvalues[idx].clone();
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?.clone();
                    match &mut *uv.0.borrow_mut() {
                        UpvalueState::Open(stack_idx) => {
                            self.stack[*stack_idx] = val;
                        }
                        UpvalueState::Closed(v) => {
                            *v = val;
                        }
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
                    let start = self
                        .stack
                        .len()
                        .checked_sub(n)
                        .ok_or(VmError::StackUnderflow)?;
                    let vals: Vec<VmValue> = self.stack.drain(start..).collect();
                    let parts: Vec<String> = vals.iter().map(|v| self.format_value(v)).collect();
                    self.stack.push(VmValue::Str(parts.concat()));
                }

                OpCode::BuildList(n) => {
                    let start = self
                        .stack
                        .len()
                        .checked_sub(n)
                        .ok_or(VmError::StackUnderflow)?;
                    let elems: Vec<VmValue> = self.stack.drain(start..).collect();
                    let v = self.alloc_list(elems);
                    self.stack.push(v);
                }

                OpCode::BuildMap(n) => {
                    // Stack layout: key0, val0, key1, val1, ... (2*n values)
                    let start = self
                        .stack
                        .len()
                        .checked_sub(n * 2)
                        .ok_or(VmError::StackUnderflow)?;
                    let flat: Vec<VmValue> = self.stack.drain(start..).collect();
                    let mut map = HashMap::new();
                    for chunk in flat.chunks(2) {
                        let key = match &chunk[0] {
                            VmValue::Str(s) => s.clone(),
                            other => {
                                return Err(VmError::TypeError {
                                    message: format!("map key must be a string, got {}", other),
                                    line,
                                });
                            }
                        };
                        map.insert(key, chunk[1].clone());
                    }
                    let v = self.alloc_map(map);
                    self.stack.push(v);
                }

                OpCode::BuildRange => {
                    let (a, b) = self.pop2()?;
                    match (&a, &b) {
                        (VmValue::Int(from), VmValue::Int(to)) => {
                            self.stack.push(VmValue::Range {
                                from: *from,
                                to: *to,
                            });
                        }
                        _ => {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "range bounds must be integers, got {} and {}",
                                    a, b
                                ),
                                line,
                            });
                        }
                    }
                }

                OpCode::Index => {
                    let idx = self.pop()?;
                    let obj = self.pop()?;
                    match (obj, &idx) {
                        (VmValue::List(r), VmValue::Int(i)) => {
                            let len = self.get_list(r).len() as i64;
                            let i = if *i < 0 { len + i } else { *i };
                            if i < 0 || i >= len {
                                return Err(VmError::TypeError {
                                    message: format!(
                                        "list index {} out of bounds (len {})",
                                        idx, len
                                    ),
                                    line,
                                });
                            }
                            let v = self.get_list(r)[i as usize].clone();
                            self.stack.push(v);
                        }
                        (VmValue::Map(r), VmValue::Str(key)) => {
                            let val = self
                                .get_map(r)
                                .get(key.as_str())
                                .cloned()
                                .unwrap_or(VmValue::Nil);
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
                        (obj, _) => {
                            return Err(VmError::TypeError {
                                message: format!("cannot index {} with {}", obj, idx),
                                line,
                            });
                        }
                    }
                }

                OpCode::IndexSet => {
                    let val = self.pop()?;
                    let idx = self.pop()?;
                    let obj = self.pop()?;
                    match (obj, idx) {
                        (VmValue::List(r), VmValue::Int(i)) => {
                            let len = self.get_list(r).len() as i64;
                            let i = if i < 0 { len + i } else { i };
                            if i < 0 || i >= len {
                                return Err(VmError::TypeError {
                                    message: format!("list index {} out of bounds", i),
                                    line,
                                });
                            }
                            self.get_list_mut(r)[i as usize] = val.clone();
                        }
                        (VmValue::Map(r), VmValue::Str(key)) => {
                            self.get_map_mut(r).insert(key, val.clone());
                        }
                        (obj, idx) => {
                            return Err(VmError::TypeError {
                                message: format!("cannot index-assign {} with {}", obj, idx),
                                line,
                            });
                        }
                    }
                    self.stack.push(val);
                }

                // ── Class opcodes ────────────────────────────────────────────
                OpCode::DefClass(desc_idx) => {
                    let (
                        class_name,
                        superclass_name,
                        superclass_dynamic,
                        is_module,
                        includes,
                        own_fields,
                        class_method_names,
                        method_names,
                        private_methods,
                        nested_class_names,
                    ) = {
                        let consts = &self.frames.last().unwrap().function.chunk.constants;
                        match &consts[desc_idx] {
                            Constant::ClassDesc {
                                name,
                                superclass,
                                superclass_dynamic,
                                is_module,
                                includes,
                                field_names,
                                field_defaults,
                                method_names,
                                private_methods,
                                class_method_names,
                                nested_class_names,
                            } => {
                                let own_fields: Vec<(String, VmValue)> = field_names
                                    .iter()
                                    .zip(field_defaults.iter())
                                    .map(|(n, d)| {
                                        (
                                            n.clone(),
                                            d.as_ref().map(VmValue::from).unwrap_or(VmValue::Nil),
                                        )
                                    })
                                    .collect();
                                (
                                    name.clone(),
                                    superclass.clone(),
                                    *superclass_dynamic,
                                    *is_module,
                                    includes.clone(),
                                    own_fields,
                                    class_method_names.clone(),
                                    method_names.clone(),
                                    private_methods.clone(),
                                    nested_class_names.clone(),
                                )
                            }
                            _ => panic!("DefClass: expected ClassDesc constant"),
                        }
                    };
                    // Pop dynamic superclass from TOS if the superclass expression was not a
                    // simple variable (e.g. `Foo.Bar`).
                    let dynamic_super: Option<String> = if superclass_dynamic {
                        match self.pop()? {
                            VmValue::Class { name, .. } => Some(name),
                            other => {
                                return Err(VmError::TypeError {
                                    message: format!("superclass must be a class, got {}", other),
                                    line,
                                });
                            }
                        }
                    } else {
                        None
                    };
                    // Drain class method closures, instance method closures, then nested
                    // class values from the stack (pushed in that order by the compiler).
                    let n_class = class_method_names.len();
                    let n_nested = nested_class_names.len();
                    let class_start = self
                        .stack
                        .len()
                        .checked_sub(n_class + method_names.len() + n_nested)
                        .ok_or(VmError::StackUnderflow)?;
                    let all_values: Vec<VmValue> = self.stack.drain(class_start..).collect();
                    let (class_closures, rest) = all_values.split_at(n_class);
                    let (instance_closures, nested_values) = rest.split_at(method_names.len());

                    let mut own_class_methods: HashMap<String, VmMethod> = HashMap::new();
                    for (mname, closure) in class_method_names.iter().zip(class_closures) {
                        match closure {
                            VmValue::Closure { function, upvalues } => {
                                own_class_methods.insert(
                                    mname.clone(),
                                    VmMethod {
                                        function: function.clone(),
                                        upvalues: upvalues.clone(),
                                        defined_in: class_name.clone(),
                                        private: false,
                                    },
                                );
                            }
                            _ => panic!("DefClass: class method is not a closure"),
                        }
                    }
                    let mut own_methods: HashMap<String, VmMethod> = HashMap::new();
                    for (mname, closure) in method_names.iter().zip(instance_closures) {
                        match closure {
                            VmValue::Closure { function, upvalues } => {
                                let private = private_methods.contains(mname);
                                own_methods.insert(
                                    mname.clone(),
                                    VmMethod {
                                        function: function.clone(),
                                        upvalues: upvalues.clone(),
                                        defined_in: class_name.clone(),
                                        private,
                                    },
                                );
                            }
                            _ => panic!("DefClass: method is not a closure"),
                        }
                    }
                    // Build namespace from nested class values.
                    let namespace: HashMap<String, VmValue> = nested_class_names
                        .iter()
                        .zip(nested_values.iter())
                        .map(|(n, v)| (n.clone(), v.clone()))
                        .collect();
                    // Resolve the effective superclass name (modules have no superclass).
                    let effective_super = if is_module {
                        None
                    } else {
                        dynamic_super.or(superclass_name).or_else(|| {
                            if class_name != "Object" && self.classes.contains_key("Object") {
                                Some("Object".to_string())
                            } else {
                                None
                            }
                        })
                    };
                    let ancestors = self.ancestor_list_for_defines(
                        &class_name,
                        effective_super.as_deref(),
                        &includes,
                        is_module,
                        line,
                    )?;
                    // Merge inherited fields, instance methods, and class methods from superclass.
                    let (merged_fields, mut merged_methods, mut merged_class_methods) =
                        if let Some(ref sname) = effective_super {
                            let (parent_fields, parent_methods, parent_class_methods) =
                                match self.classes.get(sname) {
                                    Some(entry) => (
                                        entry.fields.clone(),
                                        (*entry.methods).clone(),
                                        (*entry.class_methods).clone(),
                                    ),
                                    None => {
                                        return Err(VmError::TypeError {
                                            message: format!("superclass '{}' not found", sname),
                                            line,
                                        });
                                    }
                                };
                            let mut mf = parent_fields;
                            mf.extend(own_fields);
                            let mm = parent_methods;
                            let mc = parent_class_methods;
                            (mf, mm, mc)
                        } else {
                            (own_fields, HashMap::new(), HashMap::new())
                        };
                    // Overlay included modules (classes only).
                    if !is_module {
                        let mut visiting = HashSet::new();
                        for inc in &includes {
                            let order = self.mixin_expansion_order(inc, &mut visiting, line)?;
                            for mname in order {
                                let mentry = self.classes.get(&mname).ok_or_else(|| {
                                    VmError::TypeError {
                                        message: format!("module '{}' not found", mname),
                                        line,
                                    }
                                })?;
                                merged_methods.extend(
                                    mentry.methods.iter().map(|(k, v)| (k.clone(), v.clone())),
                                );
                                merged_class_methods.extend(
                                    mentry
                                        .class_methods
                                        .iter()
                                        .map(|(k, v)| (k.clone(), v.clone())),
                                );
                            }
                        }
                    }
                    merged_methods.extend(own_methods);
                    merged_class_methods.extend(own_class_methods);
                    let merged_rc = Rc::new(merged_methods);
                    let merged_class_rc = Rc::new(merged_class_methods);
                    let namespace_rc = Rc::new(namespace);
                    self.classes.insert(
                        class_name.clone(),
                        ClassEntry {
                            superclass: effective_super.clone(),
                            ancestors,
                            includes: includes.clone(),
                            is_module,
                            fields: merged_fields.clone(),
                            methods: merged_rc.clone(),
                            class_methods: merged_class_rc.clone(),
                            namespace: namespace_rc.clone(),
                        },
                    );
                    // Mirror bytecode methods into the bootstrapped ClassObject if
                    // this is a core class (Object, Class, Set, …).
                    if let Some(class_obj_ref) = self.find_core_class_obj(&class_name) {
                        for (mname, vm_method) in merged_rc.iter() {
                            if let HeapObject::ClassObject { methods, .. } =
                                self.heap.get_mut(class_obj_ref)
                            {
                                // Bootstrapped natives (e.g. Set#to_s) win over inherited
                                // bytecode from Object unless this class defines its own method.
                                if matches!(methods.get(mname), Some(SapphireMethod::Native { .. }))
                                    && vm_method.defined_in != class_name
                                {
                                    continue;
                                }
                                methods.insert(
                                    mname.clone(),
                                    SapphireMethod::Bytecode(vm_method.clone()),
                                );
                            }
                        }
                    }
                    self.stack.push(VmValue::Class {
                        name: class_name,
                        superclass: effective_super,
                        fields: merged_fields,
                        methods: merged_rc,
                        class_methods: merged_class_rc,
                        namespace: namespace_rc,
                    });
                }

                OpCode::NewInstance(n_pairs) => {
                    // Stack: [class, name0, val0, …, nameN, valN]
                    let base = self
                        .stack
                        .len()
                        .checked_sub(1 + n_pairs * 2)
                        .ok_or(VmError::StackUnderflow)?;
                    let (class_name, field_decls, methods, ancestor_chain) =
                        match &self.stack[base] {
                            VmValue::Class {
                                name,
                                fields,
                                methods,
                                ..
                            } => {
                                let anc = self
                                    .classes
                                    .get(name)
                                    .map(|e| Rc::new(e.ancestors.clone()))
                                    .unwrap_or_else(|| Rc::new(vec![name.clone()]));
                                (
                                    name.clone(),
                                    fields.clone(),
                                    methods.clone(),
                                    anc,
                                )
                            }
                            VmValue::ClassObj(r) => {
                                let r = *r;
                                let name = match self.heap.get(r) {
                                    HeapObject::ClassObject { name, .. } => name.clone(),
                                    _ => unreachable!(),
                                };
                                match self.classes.get(&name) {
                                    Some(entry) => (
                                        name,
                                        entry.fields.clone(),
                                        entry.methods.clone(),
                                        Rc::new(entry.ancestors.clone()),
                                    ),
                                    None => (
                                        name,
                                        vec![],
                                        Rc::new(HashMap::new()),
                                        Rc::new(Vec::new()),
                                    ),
                                }
                            }
                            other => {
                                return Err(VmError::TypeError {
                                    message: format!("'{}' is not a class", other),
                                    line,
                                });
                            }
                        };
                    if self
                        .classes
                        .get(&class_name)
                        .map(|e| e.is_module)
                        .unwrap_or(false)
                    {
                        let msg =
                            VmValue::Str(format!("cannot instantiate module '{}'", class_name));
                        self.raise_value(msg)?;
                        continue;
                    }
                    // Regex.new("pattern") / Regex.new("pattern", ignore_case: true)
                    if class_name == "Regex" {
                        let pattern = match self.stack.get(base + 2) {
                            Some(VmValue::Str(s)) => s.clone(),
                            Some(_) => {
                                return Err(VmError::TypeError {
                                    message: "Regex.new expects a String pattern".to_string(),
                                    line,
                                });
                            }
                            None => {
                                return Err(VmError::TypeError {
                                    message: "Regex.new requires a pattern argument".to_string(),
                                    line,
                                });
                            }
                        };
                        let mut ignore_case = false;
                        for i in 1..n_pairs {
                            if let Some(VmValue::Str(name)) = self.stack.get(base + 1 + i * 2)
                                && name == "ignore_case"
                                && let Some(VmValue::Bool(b)) = self.stack.get(base + 2 + i * 2)
                            {
                                ignore_case = *b;
                            }
                        }
                        let re = crate::native_regex::build_regex(&pattern, ignore_case, line)?;
                        let id = self.next_regex_id;
                        self.next_regex_id += 1;
                        self.regexes.insert(id, re);
                        let methods = self
                            .classes
                            .get("Regex")
                            .map(|e| e.methods.clone())
                            .ok_or_else(|| VmError::TypeError {
                                message: "Regex class not loaded".to_string(),
                                line,
                            })?;
                        let mut fields = HashMap::new();
                        fields.insert("id".to_string(), VmValue::Int(id));
                        let gc_fields = self.alloc_fields(fields);
                        self.stack.drain(base..);
                        self.stack.push(VmValue::Instance {
                            class_name: "Regex".to_string(),
                            ancestor_chain: Rc::new(
                                self.classes
                                    .get("Regex")
                                    .map(|e| e.ancestors.clone())
                                    .unwrap_or_else(|| vec!["Regex".to_string()]),
                            ),
                            fields: gc_fields,
                            methods,
                        });
                        continue;
                    }
                    // Set.new() / Set.new([items]) — intercept before normal instance allocation.
                    if class_name == "Set" {
                        let list_val = if n_pairs == 0 {
                            None
                        } else {
                            Some(self.stack[base + 2].clone())
                        };
                        let elements = match list_val {
                            None => Vec::new(),
                            Some(VmValue::List(lr)) => {
                                crate::native_set::dedup_list(self.heap.get_list(lr).clone())
                            }
                            _ => {
                                return Err(VmError::TypeError {
                                    message: "Set.new expects a List argument".to_string(),
                                    line,
                                });
                            }
                        };
                        self.stack.drain(base..);
                        let result = self.alloc_set(elements);
                        self.stack.push(result);
                        continue;
                    }
                    // Initialise fields to their declared defaults (or nil if none).
                    let mut instance_fields: HashMap<String, VmValue> = field_decls
                        .iter()
                        .map(|(n, default)| (n.clone(), default.clone()))
                        .collect();
                    // Apply named constructor arguments.
                    for i in 0..n_pairs {
                        let name_val = self.stack[base + 1 + i * 2].clone();
                        let val = self.stack[base + 2 + i * 2].clone();
                        match name_val {
                            VmValue::Str(ref n) if !n.is_empty() => {
                                instance_fields.insert(n.clone(), val);
                            }
                            _ => {}
                        }
                    }
                    self.stack.drain(base..);
                    let fields = self.alloc_fields(instance_fields);
                    self.stack.push(VmValue::Instance {
                        class_name,
                        fields,
                        methods,
                        ancestor_chain,
                    });
                }

                OpCode::GetField(idx) => {
                    let name = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Str(s) => s.clone(),
                        _ => panic!("GetField: expected Str constant"),
                    };
                    let obj = self.pop()?;
                    match obj {
                        VmValue::Instance { fields, .. } => {
                            let val = self
                                .get_fields(fields)
                                .get(&name)
                                .cloned()
                                .unwrap_or(VmValue::Nil);
                            self.stack.push(val);
                        }
                        // Namespace lookup: `Outer.Inner` where `Inner` is a nested class.
                        VmValue::Class { ref namespace, .. } => match namespace.get(&name) {
                            Some(val) => {
                                self.stack.push(val.clone());
                            }
                            None => {
                                return Err(VmError::TypeError {
                                    message: format!(
                                        "class has no nested class or attribute '{}'",
                                        name
                                    ),
                                    line,
                                });
                            }
                        },
                        // For primitives, treat `obj.name` as a zero-arg method call.
                        ref other => {
                            let method = primitive_class_name(other)
                                .and_then(|cls| self.classes.get(cls))
                                .and_then(|entry| entry.methods.get(&name).cloned());
                            match method {
                                Some(m) => {
                                    let recv_slot = self.stack.len();
                                    self.stack.push(other.clone());
                                    let class_name = Some(m.defined_in.clone());
                                    self.frames.push(CallFrame {
                                        function: m.function,
                                        upvalues: m.upvalues,
                                        ip: 0,
                                        base: recv_slot,
                                        block: None,
                                        is_block_caller: false,
                                        is_native_block: false,
                                        rescues: vec![],
                                        class_name,
                                    });
                                }
                                None => {
                                    return Err(VmError::TypeError {
                                        message: format!(
                                            "cannot get field '{}' on {}",
                                            name, other
                                        ),
                                        line,
                                    });
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
                        VmValue::Instance { fields, .. } => {
                            let val = self
                                .get_fields(fields)
                                .get(&name)
                                .cloned()
                                .unwrap_or(VmValue::Nil);
                            self.stack.push(val);
                        }
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("cannot get field '{}' on {}", name, other),
                                line,
                            });
                        }
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
                        VmValue::Instance { fields, .. } => {
                            self.get_fields_mut(fields).insert(name, val.clone());
                            self.stack.push(val);
                        }
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("cannot set field '{}' on {}", name, other),
                                line,
                            });
                        }
                    }
                }

                OpCode::Invoke(name_idx, arg_count) => {
                    let method_name =
                        match &self.frames.last().unwrap().function.chunk.constants[name_idx] {
                            Constant::Str(s) => s.clone(),
                            _ => panic!("Invoke: expected Str constant for method name"),
                        };
                    let recv_slot = self
                        .stack
                        .len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;

                    if method_name == "is_a?" && arg_count == 1 {
                        let recv = self.stack[recv_slot].clone();
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        let result =
                            invoke_is_a(&self.heap, &self.classes, &recv, &args, line)?;
                        self.stack.truncate(recv_slot);
                        self.stack.push(result);
                        continue;
                    }

                    if method_name == "class" && arg_count == 0 {
                        let recv = self.stack[recv_slot].clone();
                        // For bootstrapped types, return the heap-allocated ClassObj.
                        let bootstrapped = match &recv {
                            VmValue::Float(_) => self.core_classes.float_cls.map(VmValue::ClassObj),
                            VmValue::Int(_) => self.core_classes.int_cls.map(VmValue::ClassObj),
                            VmValue::Nil => self.core_classes.nil_cls.map(VmValue::ClassObj),
                            VmValue::Set(_) => self.core_classes.set_cls.map(VmValue::ClassObj),
                            VmValue::Str(_) => self.core_classes.string_cls.map(VmValue::ClassObj),
                            VmValue::Range { .. } => self.core_classes.range_cls.map(VmValue::ClassObj),
                            VmValue::List(_) => self.core_classes.list_cls.map(VmValue::ClassObj),
                            VmValue::Map(_) => self.core_classes.map_cls.map(VmValue::ClassObj),
                            VmValue::ClassObj(r) => {
                                let r = *r;
                                if let HeapObject::ClassObject { class_ref: Some(cr), .. } =
                                    self.heap.get(r)
                                {
                                    Some(VmValue::ClassObj(*cr))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        let result = if let Some(v) = bootstrapped {
                            v
                        } else {
                            match starting_class_name_for_is_a(&recv) {
                                Some(cname) => match self.classes.get(&cname) {
                                    Some(entry) => VmValue::Class {
                                        name: cname,
                                        superclass: entry.superclass.clone(),
                                        fields: entry.fields.clone(),
                                        methods: entry.methods.clone(),
                                        class_methods: entry.class_methods.clone(),
                                        namespace: entry.namespace.clone(),
                                    },
                                    None => VmValue::Nil,
                                },
                                None => VmValue::Nil,
                            }
                        };
                        self.stack.truncate(recv_slot);
                        self.stack.push(result);
                        continue;
                    }

                    // Lambda `.call(args)` — invoke the closure as a new frame.
                    if method_name == "call"
                        && let VmValue::Closure { function, upvalues } =
                            self.stack[recv_slot].clone()
                    {
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
                            ip: 0,
                            base: recv_slot,
                            block: None,
                            is_block_caller: false,
                            is_native_block: false,
                            rescues: vec![],
                            class_name: None,
                        });
                        continue;
                    }

                    // ClassObj method dispatch: receiver is a bootstrapped ClassObject.
                    if let VmValue::ClassObj(class_obj_ref) = self.stack[recv_slot].clone() {
                        let result = if method_name == "name" && arg_count == 0 {
                            let name = match self.heap.get(class_obj_ref) {
                                HeapObject::ClassObject { name, .. } => name.clone(),
                                _ => unreachable!(),
                            };
                            VmValue::Str(name)
                        } else if method_name == "superclass" && arg_count == 0 {
                            let superclass_ref = match self.heap.get(class_obj_ref) {
                                HeapObject::ClassObject { superclass, .. } => *superclass,
                                _ => unreachable!(),
                            };
                            match superclass_ref {
                                Some(r) => VmValue::ClassObj(r),
                                None => VmValue::Nil,
                            }
                        } else {
                            return Err(VmError::TypeError {
                                message: format!("unknown class method '{}'", method_name),
                                line,
                            });
                        };
                        self.stack.truncate(recv_slot);
                        self.stack.push(result);
                        continue;
                    }

                    // Class method dispatch: receiver is a Class value.
                    if let VmValue::Class {
                        ref class_methods,
                        ref namespace,
                        ref name,
                        ref methods,
                        ref superclass,
                        ..
                    } = self.stack[recv_slot].clone()
                    {
                        let method_opt = class_methods.get(&method_name).cloned();
                        if let Some(method) = method_opt {
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
                                function: method.function,
                                upvalues: method.upvalues,
                                ip: 0,
                                base: recv_slot,
                                block: None,
                                is_block_caller: false,
                                is_native_block: false,
                                rescues: vec![],
                                class_name: Some(method.defined_in),
                            });
                        } else if let Some(core_ref) = self.find_core_class_obj(name.as_str())
                            && let Some(m) =
                                self.lookup_class_object_class_method(core_ref, &method_name)
                        {
                            let recv = self.stack[recv_slot].clone();
                            let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                            match m {
                                SapphireMethod::Native {
                                    min_arity,
                                    max_arity,
                                    func,
                                } => {
                                    if arg_count < min_arity
                                        || (max_arity != NativeArity::VARIADIC_MAX
                                            && arg_count > max_arity)
                                    {
                                        let expected = if min_arity == max_arity {
                                            format!("{}", min_arity)
                                        } else if max_arity == NativeArity::VARIADIC_MAX {
                                            format!("at least {}", min_arity)
                                        } else {
                                            format!("{} to {}", min_arity, max_arity)
                                        };
                                        return Err(VmError::TypeError {
                                            message: format!(
                                                "class method '{}' expects {} arg(s), got {}",
                                                method_name, expected, arg_count
                                            ),
                                            line,
                                        });
                                    }
                                    match func(&mut self.heap, &recv, &args, line) {
                                        Ok(val) => {
                                            self.stack.truncate(recv_slot);
                                            self.stack.push(val);
                                        }
                                        Err(VmError::Raised(val)) => {
                                            self.stack.truncate(recv_slot);
                                            self.raise_value(val)?;
                                            continue;
                                        }
                                        Err(e) => return Err(e),
                                    }
                                }
                                SapphireMethod::Bytecode(m) => {
                                    if m.private {
                                        let caller_class = self
                                            .frames
                                            .last()
                                            .and_then(|f| f.class_name.as_deref())
                                            .unwrap_or("");
                                        if caller_class != m.defined_in {
                                            return Err(VmError::TypeError {
                                                message: format!(
                                                    "private class method '{}' called from outside class",
                                                    method_name
                                                ),
                                                line,
                                            });
                                        }
                                    }
                                    if m.function.arity != arg_count {
                                        return Err(VmError::TypeError {
                                            message: format!(
                                                "class method '{}' expects {} arg(s), got {}",
                                                method_name, m.function.arity, arg_count
                                            ),
                                            line,
                                        });
                                    }
                                    self.frames.push(CallFrame {
                                        function: m.function,
                                        upvalues: m.upvalues,
                                        ip: 0,
                                        base: recv_slot,
                                        block: None,
                                        is_block_caller: false,
                                        is_native_block: false,
                                        rescues: vec![],
                                        class_name: Some(m.defined_in),
                                    });
                                }
                            }
                        } else if arg_count == 0 && let Some(val) = namespace.get(&method_name) {
                            // Namespace lookup: constants and nested classes (e.g. Math.PI, Outer.Inner).
                            self.stack.truncate(recv_slot);
                            self.stack.push(val.clone());
                        } else if name == "Math" {
                            let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                            let result = crate::native_math::dispatch_math_class_method(&method_name, &args, line)?;
                            self.stack.truncate(recv_slot);
                            self.stack.push(result);
                        } else if name == "Process" {
                            let proc_args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                            let proc_result = match crate::native_process::dispatch_process_class_method(
                                &method_name,
                                &proc_args,
                                line,
                            ) {
                                Ok(r) => r,
                                Err(VmError::Raised(val)) => {
                                    self.stack.truncate(recv_slot);
                                    self.raise_value(val)?;
                                    continue;
                                }
                                Err(e) => return Err(e),
                            };
                            let result = match proc_result {
                                crate::native_process::ProcessResult::Primitive(v) => v,
                                crate::native_process::ProcessResult::List(items) => {
                                    self.alloc_list(items)
                                }
                                crate::native_process::ProcessResult::RunOutput {
                                    stdout,
                                    stderr,
                                    exit_code,
                                } => {
                                    let methods = self
                                        .classes
                                        .get("Result")
                                        .map(|e| e.methods.clone())
                                        .ok_or_else(|| VmError::TypeError {
                                            message: "Process.Result class not loaded".to_string(),
                                            line,
                                        })?;
                                    let mut fields = HashMap::new();
                                    fields.insert("stdout".to_string(), VmValue::Str(stdout));
                                    fields.insert("stderr".to_string(), VmValue::Str(stderr));
                                    fields.insert("exit_code".to_string(), VmValue::Int(exit_code));
                                    let gc_fields = self.alloc_fields(fields);
                                    VmValue::Instance {
                                        class_name: "Result".to_string(),
                                        ancestor_chain: Rc::new(
                                            self.classes
                                                .get("Result")
                                                .map(|e| e.ancestors.clone())
                                                .unwrap_or_else(|| vec!["Result".to_string()]),
                                        ),
                                        fields: gc_fields,
                                        methods,
                                    }
                                }
                            };
                            self.stack.truncate(recv_slot);
                            self.stack.push(result);
                        } else if name == "Set" {
                            let set_args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                            let result = match (method_name.as_str(), set_args.as_slice()) {
                                ("new", []) => self.alloc_set(Vec::new()),
                                ("new", [VmValue::List(list_ref)]) => {
                                    let list_ref = *list_ref;
                                    let items: Vec<VmValue> =
                                        self.heap.get_list(list_ref).clone();
                                    let mut elements: Vec<VmValue> = Vec::new();
                                    for item in items {
                                        if !elements.contains(&item) {
                                            elements.push(item);
                                        }
                                    }
                                    self.alloc_set(elements)
                                }
                                ("new", [VmValue::Set(set_ref)]) => {
                                    let set_ref = *set_ref;
                                    let items: Vec<VmValue> =
                                        self.heap.get_set(set_ref).clone();
                                    self.alloc_set(items)
                                }
                                _ => {
                                    return Err(VmError::TypeError {
                                        message: format!(
                                            "Set.{} is not defined or wrong argument types",
                                            method_name
                                        ),
                                        line,
                                    });
                                }
                            };
                            self.stack.truncate(recv_slot);
                            self.stack.push(result);
                        } else if name == "Socket" {
                            let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                            let result = match self.dispatch_socket_class(&method_name, &args, line) {
                                Ok(val) => val,
                                Err(VmError::Raised(val)) => {
                                    self.stack.truncate(recv_slot);
                                    self.raise_value(val)?;
                                    continue;
                                }
                                Err(e) => return Err(e),
                            };
                            self.stack.truncate(recv_slot);
                            self.stack.push(result);
                        } else if matches!(
                            name.as_str(),
                            "Instant"
                                | "Date"
                                | "Time"
                                | "DateTime"
                                | "ZonedDateTime"
                                | "Duration"
                        ) {
                            let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                            let dt = match crate::datetime::dispatch_class_method(
                                &self.heap,
                                name,
                                &method_name,
                                &args,
                                line,
                            ) {
                                Ok(v) => v,
                                Err(VmError::Raised(val)) => {
                                    self.stack.truncate(recv_slot);
                                    self.raise_value(val)?;
                                    continue;
                                }
                                Err(e) => return Err(e),
                            };
                            let result = self.finalize_dt(dt, line)?;
                            self.stack.truncate(recv_slot);
                            self.stack.push(result);
                        } else if method_name == "name" && arg_count == 0 {
                            let result = VmValue::Str(name.clone());
                            self.stack.truncate(recv_slot);
                            self.stack.push(result);
                        } else if method_name == "superclass" && arg_count == 0 {
                            let result = match superclass.as_deref() {
                                Some(sname) => match self.classes.get(sname) {
                                    Some(entry) => VmValue::Class {
                                        name: sname.to_string(),
                                        superclass: entry.superclass.clone(),
                                        fields: entry.fields.clone(),
                                        methods: entry.methods.clone(),
                                        class_methods: entry.class_methods.clone(),
                                        namespace: entry.namespace.clone(),
                                    },
                                    None => VmValue::Nil,
                                },
                                None => VmValue::Nil,
                            };
                            self.stack.truncate(recv_slot);
                            self.stack.push(result);
                        } else if method_name == "instance_method_names" && arg_count == 0 {
                            let mut names: Vec<VmValue> = methods
                                .iter()
                                .filter(|(_, m)| !m.private)
                                .map(|(k, _)| VmValue::Str(k.clone()))
                                .collect();
                            names.sort_by(vm_value_partial_cmp);
                            let result = VmValue::List(self.heap.alloc(HeapObject::List(names)));
                            self.stack.truncate(recv_slot);
                            self.stack.push(result);
                        } else {
                            return Err(VmError::TypeError {
                                message: format!("unknown class method '{}'", method_name),
                                line,
                            });
                        }
                        continue;
                    }

                    // Try native dispatch for non-Instance types.
                    let is_instance = matches!(&self.stack[recv_slot], VmValue::Instance { .. });
                    if !is_instance {
                        let recv = self.stack[recv_slot].clone();
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        // ClassObject method table (native + mirrored bytecode) for
                        // bootstrapped primitives before the legacy native dispatch path.
                        if let Some(start) = self.class_object_for_primitive(&recv)
                            && let Some(m) =
                                self.lookup_class_object_method(start, &method_name)
                        {
                            match m {
                                SapphireMethod::Native {
                                    min_arity,
                                    max_arity,
                                    func,
                                } => {
                                    if arg_count < min_arity
                                        || (max_arity != NativeArity::VARIADIC_MAX
                                            && arg_count > max_arity)
                                    {
                                        let expected = if min_arity == max_arity {
                                            format!("{}", min_arity)
                                        } else if max_arity == NativeArity::VARIADIC_MAX {
                                            format!("at least {}", min_arity)
                                        } else {
                                            format!("{} to {}", min_arity, max_arity)
                                        };
                                        return Err(VmError::TypeError {
                                            message: format!(
                                                "method '{}' expects {} arg(s), got {}",
                                                method_name, expected, arg_count
                                            ),
                                            line,
                                        });
                                    }
                                    let result = func(&mut self.heap, &recv, &args, line)?;
                                    self.stack.truncate(recv_slot);
                                    self.stack.push(result);
                                    continue;
                                }
                                SapphireMethod::Bytecode(m) => {
                                    if m.private {
                                        let caller_class = self
                                            .frames
                                            .last()
                                            .and_then(|f| f.class_name.as_deref())
                                            .unwrap_or("");
                                        if caller_class != m.defined_in {
                                            return Err(VmError::TypeError {
                                                message: format!(
                                                    "private method '{}' called from outside class",
                                                    method_name
                                                ),
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
                                    let class_name = Some(m.defined_in.clone());
                                    self.frames.push(CallFrame {
                                        function: m.function,
                                        upvalues: m.upvalues,
                                        ip: 0,
                                        base: recv_slot,
                                        block: None,
                                        is_block_caller: false,
                                        is_native_block: false,
                                        rescues: vec![],
                                        class_name,
                                    });
                                    continue;
                                }
                            }
                        }
                        // Look in the class registry (stdlib bytecode on primitives).
                        let method = primitive_class_name(&recv)
                            .and_then(|cls| self.classes.get(cls))
                            .and_then(|entry| entry.methods.get(&method_name).cloned());
                        match method {
                            Some(m) => {
                                if m.private {
                                    let caller_class = self
                                        .frames
                                        .last()
                                        .and_then(|f| f.class_name.as_deref())
                                        .unwrap_or("");
                                    if caller_class != m.defined_in {
                                        return Err(VmError::TypeError {
                                            message: format!(
                                                "private method '{}' called from outside class",
                                                method_name
                                            ),
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
                                    ip: 0,
                                    base: recv_slot,
                                    block: None,
                                    is_block_caller: false,
                                    is_native_block: false,
                                    rescues: vec![],
                                    class_name,
                                });
                                continue;
                            }
                            None => {
                                return Err(VmError::TypeError {
                                    message: format!("{} ({}) has no method '{}'", recv, value_type_name(&recv), method_name),
                                    line,
                                });
                            }
                        }
                    }

                    let (method_opt, dt_class, dt_fields) = match &self.stack[recv_slot] {
                        VmValue::Instance { class_name, methods, fields, .. } => (
                            methods.get(&method_name).cloned(),
                            class_name.clone(),
                            *fields,
                        ),
                        _ => unreachable!(),
                    };

                    // For datetime types, try native dispatch first so our
                    // implementations shadow inherited Object methods (e.g. to_s).
                    let mut dt_handled = false;
                    if matches!(
                        dt_class.as_str(),
                        "Instant"
                            | "Date"
                            | "Time"
                            | "DateTime"
                            | "ZonedDateTime"
                            | "Duration"
                    ) {
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        match crate::datetime::dispatch_instance_method(
                            &self.heap,
                            &dt_class,
                            dt_fields,
                            &method_name,
                            &args,
                            line,
                        ) {
                            Ok(dt) => {
                                let result = self.finalize_dt(dt, line)?;
                                self.stack.truncate(recv_slot);
                                self.stack.push(result);
                                dt_handled = true;
                            }
                            Err(VmError::Raised(val)) => {
                                self.stack.truncate(recv_slot);
                                self.raise_value(val)?;
                                continue;
                            }
                            Err(VmError::TypeError { .. }) => {
                                // Method not found natively; fall through to
                                // compiled dispatch (e.g. user-defined methods).
                            }
                            Err(e) => return Err(e),
                        }
                    }

                    let mut regex_handled = false;
                    if dt_class == "Regex" {
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        match self.dispatch_regex_instance(dt_fields, &method_name, &args, line) {
                            Ok(result) => {
                                self.stack.truncate(recv_slot);
                                self.stack.push(result);
                                regex_handled = true;
                            }
                            Err(VmError::Raised(val)) => {
                                self.stack.truncate(recv_slot);
                                self.raise_value(val)?;
                                continue;
                            }
                            Err(e) => return Err(e),
                        }
                    }

                    let mut socket_handled = false;
                    if dt_class == "Socket" {
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        match self.dispatch_socket_instance(dt_fields, &method_name, &args, line) {
                            Ok(result) => {
                                self.stack.truncate(recv_slot);
                                self.stack.push(result);
                                socket_handled = true;
                            }
                            Err(VmError::Raised(val)) => {
                                self.stack.truncate(recv_slot);
                                self.raise_value(val)?;
                                continue;
                            }
                            Err(e) => return Err(e),
                        }
                    }

                    if regex_handled || socket_handled || dt_handled {
                        // already handled above
                    } else if let Some(method) = method_opt {
                        if method.private {
                            let caller_class = self
                                .frames
                                .last()
                                .and_then(|f| f.class_name.as_deref())
                                .unwrap_or("");
                            if caller_class != method.defined_in {
                                return Err(VmError::TypeError {
                                    message: format!(
                                        "private method '{}' called from outside class",
                                        method_name
                                    ),
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
                            ip: 0,
                            base: recv_slot,
                            block: None,
                            is_block_caller: false,
                            is_native_block: false,
                            rescues: vec![],
                            class_name,
                        });
                    } else if arg_count == 0 {
                        // No method found — fall back to field read (attr-declared fields accessed without parens)
                        let field_val = match &self.stack[recv_slot] {
                            VmValue::Instance { fields, .. } => {
                                let r = *fields;
                                self.get_fields(r).get(&method_name).cloned()
                            }
                            _ => unreachable!(),
                        };
                        match field_val {
                            Some(val) => {
                                self.stack.truncate(recv_slot);
                                self.stack.push(val);
                            }
                            None => {
                                return Err(VmError::TypeError {
                                    message: format!(
                                        "undefined method or field '{}' not found",
                                        method_name
                                    ),
                                    line,
                                });
                            }
                        }
                    } else {
                        return Err(VmError::TypeError {
                            message: format!("method '{}' not found", method_name),
                            line,
                        });
                    }
                }

                OpCode::SuperInvoke(name_idx, arg_count) => {
                    let method_name =
                        match &self.frames.last().unwrap().function.chunk.constants[name_idx] {
                            Constant::Str(s) => s.clone(),
                            _ => panic!("SuperInvoke: expected Str constant"),
                        };
                    let recv_slot = self
                        .stack
                        .len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;
                    let chain = match &self.stack[recv_slot] {
                        VmValue::Instance { ancestor_chain, .. } => ancestor_chain.as_slice(),
                        other => {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "super requires an instance receiver, got {}",
                                    other
                                ),
                                line,
                            });
                        }
                    };
                    let current_class =
                        self.frames
                            .last()
                            .unwrap()
                            .class_name
                            .clone()
                            .ok_or_else(|| VmError::TypeError {
                                message: "super used outside of a method".into(),
                                line,
                            })?;
                    let pos = chain
                        .iter()
                        .position(|a| a == &current_class)
                        .ok_or_else(|| VmError::TypeError {
                            message: format!(
                                "internal error: '{}' not in receiver ancestor chain",
                                current_class
                            ),
                            line,
                        })?;
                    let super_name = chain
                        .get(pos + 1)
                        .cloned()
                        .ok_or_else(|| VmError::TypeError {
                            message: format!("'{}' has no superclass", current_class),
                            line,
                        })?;
                    let method = match self.classes.get(&super_name) {
                        Some(entry) => {
                            entry.methods.get(&method_name).cloned().ok_or_else(|| {
                                VmError::TypeError {
                                    message: format!(
                                        "ancestor '{}' has no method '{}'",
                                        super_name, method_name
                                    ),
                                    line,
                                }
                            })?
                        }
                        None => {
                            return Err(VmError::TypeError {
                                message: format!("ancestor '{}' not in registry", super_name),
                                line,
                            });
                        }
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
                    let super_dispatch_class = method.defined_in.clone();
                    self.frames.push(CallFrame {
                        function: method.function,
                        upvalues: method.upvalues,
                        ip: 0,
                        base: recv_slot,
                        block: None,
                        is_block_caller: false,
                        is_native_block: false,
                        rescues: vec![],
                        class_name: Some(super_dispatch_class),
                    });
                }

                OpCode::GetSelf => {
                    let base = self.frames.last().unwrap().base;
                    let val = self.stack[base].clone();
                    self.stack.push(val);
                }

                OpCode::GetLexicalConstant(idx) => {
                    let consts = &self.frames.last().unwrap().function.chunk.constants;
                    let (enclosing, const_name) = match &consts[idx] {
                        Constant::LexicalClassScope {
                            enclosing_classes,
                            name_idx,
                        } => {
                            let cn = match &consts[*name_idx] {
                                Constant::Str(s) => s.clone(),
                                _ => {
                                    return Err(VmError::TypeError {
                                        message: "GetLexicalConstant: expected string name"
                                            .to_string(),
                                        line,
                                    });
                                }
                            };
                            (enclosing_classes.clone(), cn)
                        }
                        _ => {
                            return Err(VmError::TypeError {
                                message: "GetLexicalConstant: expected LexicalClassScope constant"
                                    .to_string(),
                                line,
                            });
                        }
                    };
                    let mut val: Option<VmValue> = None;
                    for class_name in enclosing.iter().rev() {
                        if let Some(entry) = self.classes.get(class_name)
                            && let Some(v) = entry.namespace.get(&const_name)
                        {
                            val = Some(v.clone());
                            break;
                        }
                    }
                    let resolved = if let Some(v) = val {
                        v
                    } else {
                        self.globals
                            .get(&const_name)
                            .cloned()
                            .ok_or_else(|| VmError::TypeError {
                                message: format!("undefined variable '{}'", const_name),
                                line,
                            })?
                    };
                    self.stack.push(resolved);
                }

                // ── Block opcodes ─────────────────────────────────────────────
                OpCode::CallWithBlock(arg_count) => {
                    // Stack: [..., fn_or_closure, arg0, …, argN-1, block_closure]
                    let block_val = self.pop()?;
                    let block = match block_val {
                        VmValue::Closure { function, upvalues } => Some(VmMethod {
                            function,
                            upvalues,
                            defined_in: String::new(),
                            private: false,
                        }),
                        VmValue::Nil => None,
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("block must be a closure, got {}", other),
                                line,
                            });
                        }
                    };
                    let fn_slot = self
                        .stack
                        .len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;
                    let (function, upvalues) = match &self.stack[fn_slot] {
                        VmValue::Function(f) => (f.clone(), vec![]),
                        VmValue::Closure { function, upvalues } => {
                            (function.clone(), upvalues.clone())
                        }
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("expected a function or method, got {}", other),
                                line,
                            });
                        }
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
                        function,
                        upvalues,
                        ip: 0,
                        base: fn_slot,
                        block,
                        is_block_caller: true,
                        is_native_block: false,
                        rescues: vec![],
                        class_name: None,
                    });
                }

                OpCode::InvokeWithBlock(name_idx, arg_count) => {
                    // Stack: [..., receiver, arg0, …, argN-1, block_closure]
                    let block_val = self.pop()?;
                    let block = match block_val {
                        VmValue::Closure { function, upvalues } => Some(VmMethod {
                            function,
                            upvalues,
                            defined_in: String::new(),
                            private: false,
                        }),
                        VmValue::Nil => None,
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("block must be a closure, got {}", other),
                                line,
                            });
                        }
                    };
                    let method_name =
                        match &self.frames.last().unwrap().function.chunk.constants[name_idx] {
                            Constant::Str(s) => s.clone(),
                            _ => panic!("InvokeWithBlock: expected Str constant"),
                        };
                    let recv_slot = self
                        .stack
                        .len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;

                    // For non-Instance receivers: VM native block dispatch (each, map, …)
                    // runs before ClassObject bytecode so bootstrapped primitives keep their
                    // Rust block semantics even after stdlib mirrors methods onto the heap.
                    let is_instance = matches!(&self.stack[recv_slot], VmValue::Instance { .. });
                    if !is_instance {
                        let recv = self.stack[recv_slot].clone();
                        let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
                        let native_result = self.dispatch_native_block_method(
                            &recv,
                            &method_name,
                            &args,
                            block.clone(),
                            line,
                        );
                        let is_native_miss = matches!(&native_result,
                            Err(VmError::TypeError { message, .. })
                            if message.contains("has no block method") || message.contains("requires a block")
                        );
                        if !is_native_miss {
                            match native_result {
                                Err(VmError::Return(val)) => {
                                    let frame = self.frames.pop().unwrap();
                                    self.close_upvalues_above(frame.base);
                                    self.stack.truncate(frame.base);
                                    if self.frames.len() <= min_depth {
                                        return Ok(val);
                                    }
                                    self.stack.push(val.unwrap_or(VmValue::Nil));
                                }
                                other => {
                                    self.stack.truncate(recv_slot);
                                    self.stack.push(other?);
                                }
                            }
                            continue;
                        }
                        if let Some(start) = self.class_object_for_primitive(&recv)
                            && let Some(SapphireMethod::Bytecode(m)) = self
                                .lookup_class_object_method(start, &method_name)
                        {
                            if m.private {
                                let caller_class = self
                                    .frames
                                    .last()
                                    .and_then(|f| f.class_name.as_deref())
                                    .unwrap_or("");
                                if caller_class != m.defined_in {
                                    return Err(VmError::TypeError {
                                        message: format!(
                                            "private method '{}' called from outside class",
                                            method_name
                                        ),
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
                            let class_name = Some(m.defined_in.clone());
                            self.frames.push(CallFrame {
                                function: m.function,
                                upvalues: m.upvalues,
                                ip: 0,
                                base: recv_slot,
                                block,
                                is_block_caller: true,
                                is_native_block: false,
                                rescues: vec![],
                                class_name,
                            });
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
                                    function: m.function,
                                    upvalues: m.upvalues,
                                    ip: 0,
                                    base: recv_slot,
                                    block,
                                    is_block_caller: true,
                                    is_native_block: false,
                                    rescues: vec![],
                                    class_name,
                                });
                                continue;
                            }
                            None => {
                                return Err(VmError::TypeError {
                                    message: format!("{} ({}) has no method '{}'", recv, value_type_name(&recv), method_name),
                                    line,
                                });
                            }
                        }
                    }

                    let method = match &self.stack[recv_slot] {
                        VmValue::Instance { methods, .. } => methods
                            .get(&method_name)
                            .cloned()
                            .ok_or_else(|| VmError::TypeError {
                                message: format!("method '{}' not found", method_name),
                                line,
                            })?,
                        _ => unreachable!(),
                    };
                    if method.private {
                        let caller_class = self
                            .frames
                            .last()
                            .and_then(|f| f.class_name.as_deref())
                            .unwrap_or("");
                        if caller_class != method.defined_in {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "private method '{}' called from outside class",
                                    method_name
                                ),
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
                        ip: 0,
                        base: recv_slot,
                        block,
                        is_block_caller: true,
                        is_native_block: false,
                        rescues: vec![],
                        class_name,
                    });
                }

                OpCode::Yield(arg_count) => {
                    // Walk up the frame stack to find the nearest block — this allows
                    // `yield` inside an inner block to call back to the enclosing method's block.
                    let block = self
                        .frames
                        .iter()
                        .rev()
                        .find_map(|f| f.block.clone())
                        .ok_or_else(|| VmError::TypeError {
                            message: "yield called without a block".into(),
                            line,
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
                    self.stack.insert(
                        args_start,
                        VmValue::Closure {
                            function: block.function.clone(),
                            upvalues: block.upvalues.clone(),
                        },
                    );
                    self.frames.push(CallFrame {
                        function: block.function,
                        upvalues: block.upvalues,
                        ip: 0,
                        base: args_start,
                        block: None,
                        is_block_caller: false,
                        is_native_block: false,
                        rescues: vec![],
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
                    // If we hit a native-block frame first, stop unwinding and
                    // propagate as an error so the native dispatch can catch it.
                    loop {
                        if let Some(frame) = self.frames.last() {
                            let is_caller = frame.is_block_caller;
                            let is_native_block = frame.is_native_block;
                            let base = frame.base;
                            self.close_upvalues_above(base);
                            self.frames.pop();
                            self.stack.truncate(base);
                            if is_caller {
                                // Push break value as the result of the call-with-block.
                                self.stack.push(val);
                                break;
                            }
                            if is_native_block {
                                // Native method called this block; let it handle break.
                                return Err(VmError::Break(val));
                            }
                        } else {
                            return Err(VmError::Break(val));
                        }
                    }
                }

                OpCode::Next => {
                    // Return from the current (block) frame immediately with `val`.
                    let val = self.pop()?;
                    let frame = self.frames.pop().unwrap();
                    self.close_upvalues_above(frame.base);
                    if self.frames.is_empty() {
                        return Ok(Some(val));
                    }
                    self.stack.truncate(frame.base);
                    self.stack.push(val);
                }

                OpCode::BeginRescue {
                    handler_offset,
                    rescue_var_slot,
                } => {
                    let handler_ip = self.frames.last().unwrap().ip + handler_offset;
                    let stack_height = self.stack.len();
                    self.frames.last_mut().unwrap().rescues.push(RescueInfo {
                        handler_ip,
                        rescue_var_slot,
                        stack_height,
                    });
                }

                OpCode::PopRescue => {
                    self.frames.last_mut().unwrap().rescues.pop();
                }

                OpCode::Print => {
                    let val = self.pop()?;
                    let s = self.format_value(&val);
                    match self.output.as_mut() {
                        Some(buf) => buf.push(s),
                        None => println!("{}", s),
                    }
                    self.stack.push(val);
                }

                OpCode::GetGlobal(idx) => {
                    let name = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Str(s) => s.clone(),
                        _ => {
                            return Err(VmError::TypeError {
                                message: "GetGlobal: expected string constant".to_string(),
                                line,
                            });
                        }
                    };
                    let val =
                        self.globals
                            .get(&name)
                            .cloned()
                            .ok_or_else(|| VmError::TypeError {
                                message: format!("undefined variable '{}'", name),
                                line,
                            })?;
                    self.stack.push(val);
                }

                OpCode::SetGlobal(idx) => {
                    let name = match &self.frames.last().unwrap().function.chunk.constants[idx] {
                        Constant::Str(s) => s.clone(),
                        _ => {
                            return Err(VmError::TypeError {
                                message: "SetGlobal: expected string constant".to_string(),
                                line,
                            });
                        }
                    };
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?.clone();
                    self.globals.insert(name, val);
                }

                OpCode::Import(path_idx) => {
                    let path_str =
                        match &self.frames.last().unwrap().function.chunk.constants[path_idx] {
                            Constant::Str(s) => s.clone(),
                            _ => {
                                return Err(VmError::TypeError {
                                    message: "import: expected string constant".into(),
                                    line,
                                });
                            }
                        };
                    if self.current_dir == PathBuf::new() {
                        return Err(VmError::TypeError {
                            message: "import is not supported in the REPL".into(),
                            line,
                        });
                    }
                    let raw_path = self.current_dir.join(&path_str).with_extension("spr");
                    let canonical = raw_path.canonicalize().map_err(|_| VmError::TypeError {
                        message: format!("import: file not found: {}", raw_path.display()),
                        line,
                    })?;
                    if !self.imported.contains(&canonical) {
                        self.imported.insert(canonical.clone());
                        let saved_dir = self.current_dir.clone();
                        self.current_dir = canonical
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| PathBuf::from("."));
                        let source = std::fs::read_to_string(&canonical).map_err(|e| {
                            VmError::TypeError {
                                message: format!(
                                    "import: could not read {}: {}",
                                    canonical.display(),
                                    e
                                ),
                                line,
                            }
                        })?;
                        let tokens = crate::lexer::Lexer::new(&source).scan_tokens();
                        let stmts = crate::parser::Parser::new(tokens).parse().map_err(|e| {
                            VmError::TypeError {
                                message: format!("import {}: {}", canonical.display(), e),
                                line: 0,
                            }
                        })?;
                        // Compile in global mode so imported classes/functions are
                        // stored as globals and remain accessible after the frame exits.
                        let func = crate::compiler::compile_repl(&stmts).map_err(|e| {
                            VmError::TypeError {
                                message: format!("import {}: {}", canonical.display(), e),
                                line: 0,
                            }
                        })?;
                        self.run_extra(func)?;
                        self.current_dir = saved_dir;
                    }
                }

                OpCode::Call(arg_count) => {
                    // Stack: [..., fn_or_closure, arg0, …, argN-1]
                    let fn_slot = self
                        .stack
                        .len()
                        .checked_sub(arg_count + 1)
                        .ok_or(VmError::StackUnderflow)?;

                    let (function, upvalues) = match &self.stack[fn_slot] {
                        VmValue::Function(f) => (f.clone(), vec![]),
                        VmValue::Closure { function, upvalues } => {
                            (function.clone(), upvalues.clone())
                        }
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("expected a function or method, got {}", other),
                                line,
                            });
                        }
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
                        function,
                        upvalues,
                        ip: 0,
                        base,
                        block: None,
                        is_block_caller: false,
                        is_native_block: false,
                        rescues: vec![],
                        class_name: None,
                    });
                }

                OpCode::Return => {
                    let return_val = self.stack.pop();
                    let frame = self.frames.pop().unwrap();

                    // Close every upvalue that points into the returning frame.
                    self.close_upvalues_above(frame.base);

                    // Enforce return type annotation if present.
                    if let Some(expected_type) = &frame.function.return_type {
                        let val = return_val.as_ref().unwrap_or(&VmValue::Nil);
                        let actual_type = value_type_name(val);
                        if !runtime_type_matches(val, expected_type) {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "return type error in '{}': expected {}, got {}",
                                    frame.function.name,
                                    runtime_type_display(expected_type),
                                    actual_type
                                ),
                                line,
                            });
                        }
                    }

                    if self.frames.len() <= min_depth {
                        return Ok(return_val);
                    }

                    // Discard the function value and all frame locals.
                    self.stack.truncate(frame.base);
                    self.stack.push(return_val.unwrap_or(VmValue::Nil));
                }

                OpCode::NonLocalReturn => {
                    let return_val = self.stack.pop();
                    let frame = self.frames.pop().unwrap();

                    self.close_upvalues_above(frame.base);

                    if let Some(expected_type) = &frame.function.return_type {
                        let val = return_val.as_ref().unwrap_or(&VmValue::Nil);
                        let actual_type = value_type_name(val);
                        if !runtime_type_matches(val, expected_type) {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "return type error in '{}': expected {}, got {}",
                                    frame.function.name,
                                    runtime_type_display(expected_type),
                                    actual_type
                                ),
                                line,
                            });
                        }
                    }

                    if frame.is_native_block {
                        return Err(VmError::Return(return_val));
                    }

                    if self.frames.len() <= min_depth {
                        return Ok(return_val);
                    }

                    self.stack.truncate(frame.base);
                    self.stack.push(return_val.unwrap_or(VmValue::Nil));
                }

                OpCode::Negate => {
                    let v = self.pop()?;
                    self.stack.push(match v {
                        VmValue::Int(n) => VmValue::Int(-n),
                        VmValue::Float(n) => VmValue::Float(-n),
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("cannot negate {}", other),
                                line,
                            });
                        }
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
                    self.stack.push(numeric_binop(
                        &a,
                        &b,
                        line,
                        "subtract",
                        |x, y| x - y,
                        |x, y| x - y,
                    )?);
                }
                OpCode::Mul => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(numeric_binop(
                        &a,
                        &b,
                        line,
                        "multiply",
                        |x, y| x * y,
                        |x, y| x * y,
                    )?);
                }
                OpCode::Div => {
                    let (a, b) = self.pop2()?;
                    let is_zero = matches!(&b, VmValue::Int(0))
                        || matches!(&b, VmValue::Float(f) if *f == 0.0);
                    if is_zero {
                        self.raise_value(VmValue::Str("division by zero".into()))?;
                        continue;
                    }
                    self.stack.push(numeric_binop(
                        &a,
                        &b,
                        line,
                        "divide",
                        |x, y| x / y,
                        |x, y| x / y,
                    )?);
                }
                OpCode::Mod => {
                    let (a, b) = self.pop2()?;
                    let is_zero = matches!(&b, VmValue::Int(0))
                        || matches!(&b, VmValue::Float(f) if *f == 0.0);
                    if is_zero {
                        self.raise_value(VmValue::Str("division by zero".into()))?;
                        continue;
                    }
                    self.stack.push(numeric_binop(
                        &a,
                        &b,
                        line,
                        "modulo",
                        |x, y| x % y,
                        |x, y| x % y,
                    )?);
                }

                OpCode::BitAnd => {
                    let (a, b) = self.pop2()?;
                    match (&a, &b) {
                        (VmValue::Int(x), VmValue::Int(y)) => self.stack.push(VmValue::Int(x & y)),
                        _ => {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "bitwise AND requires integers, got {} and {}",
                                    a, b
                                ),
                                line,
                            });
                        }
                    }
                }
                OpCode::BitOr => {
                    let (a, b) = self.pop2()?;
                    match (&a, &b) {
                        (VmValue::Int(x), VmValue::Int(y)) => self.stack.push(VmValue::Int(x | y)),
                        _ => {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "bitwise OR requires integers, got {} and {}",
                                    a, b
                                ),
                                line,
                            });
                        }
                    }
                }
                OpCode::BitXor => {
                    let (a, b) = self.pop2()?;
                    match (&a, &b) {
                        (VmValue::Int(x), VmValue::Int(y)) => self.stack.push(VmValue::Int(x ^ y)),
                        _ => {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "bitwise XOR requires integers, got {} and {}",
                                    a, b
                                ),
                                line,
                            });
                        }
                    }
                }
                OpCode::BitNot => {
                    let v = self.pop()?;
                    match v {
                        VmValue::Int(n) => self.stack.push(VmValue::Int(!n)),
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("bitwise NOT requires an integer, got {}", other),
                                line,
                            });
                        }
                    }
                }
                OpCode::Shl => {
                    let (a, b) = self.pop2()?;
                    match (&a, &b) {
                        (VmValue::Int(x), VmValue::Int(y)) => self.stack.push(VmValue::Int(x << y)),
                        _ => {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "left shift requires integers, got {} and {}",
                                    a, b
                                ),
                                line,
                            });
                        }
                    }
                }
                OpCode::Shr => {
                    let (a, b) = self.pop2()?;
                    match (&a, &b) {
                        (VmValue::Int(x), VmValue::Int(y)) => self.stack.push(VmValue::Int(x >> y)),
                        _ => {
                            return Err(VmError::TypeError {
                                message: format!(
                                    "right shift requires integers, got {} and {}",
                                    a, b
                                ),
                                line,
                            });
                        }
                    }
                }

                OpCode::Equal => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(VmValue::Bool(a == b));
                }
                OpCode::NotEqual => {
                    let (a, b) = self.pop2()?;
                    self.stack.push(VmValue::Bool(a != b));
                }

                OpCode::Len => {
                    let val = self.pop()?;
                    let n = match &val {
                        VmValue::List(r) => self.get_list(*r).len() as i64,
                        VmValue::Map(r) => self.get_map(*r).len() as i64,
                        VmValue::Str(s) => s.chars().count() as i64,
                        VmValue::Range { from, to } => (to - from).max(0),
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("size not supported for {}", other),
                                line,
                            });
                        }
                    };
                    self.stack.push(VmValue::Int(n));
                }

                OpCode::MapKeys => {
                    let val = self.pop()?;
                    let mut keys = match val {
                        VmValue::Map(r) => self.get_map(r).keys().cloned().collect::<Vec<_>>(),
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("map_keys() not supported for {}", other),
                                line,
                            });
                        }
                    };
                    keys.sort();
                    let list = keys.into_iter().map(VmValue::Str).collect();
                    let v = self.alloc_list(list);
                    self.stack.push(v);
                }

                OpCode::RangeFrom => {
                    let val = self.pop()?;
                    match val {
                        VmValue::Range { from, .. } => self.stack.push(VmValue::Int(from)),
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("range_from() not supported for {}", other),
                                line,
                            });
                        }
                    }
                }

                OpCode::RangeTo => {
                    let val = self.pop()?;
                    match val {
                        VmValue::Range { to, .. } => self.stack.push(VmValue::Int(to)),
                        other => {
                            return Err(VmError::TypeError {
                                message: format!("range_to() not supported for {}", other),
                                line,
                            });
                        }
                    }
                }

                OpCode::Less => {
                    let (a, b) = self.pop2()?;
                    self.stack
                        .push(VmValue::Bool(numeric_cmp(&a, &b, line, |x, y| x < y)?));
                }
                OpCode::LessEqual => {
                    let (a, b) = self.pop2()?;
                    self.stack
                        .push(VmValue::Bool(numeric_cmp(&a, &b, line, |x, y| x <= y)?));
                }
                OpCode::Greater => {
                    let (a, b) = self.pop2()?;
                    self.stack
                        .push(VmValue::Bool(numeric_cmp(&a, &b, line, |x, y| x > y)?));
                }
                OpCode::GreaterEqual => {
                    let (a, b) = self.pop2()?;
                    self.stack
                        .push(VmValue::Bool(numeric_cmp(&a, &b, line, |x, y| x >= y)?));
                }
            }
        }
    }

    // ── Upvalue helpers ───────────────────────────────────────────────────────

    /// Return an open upvalue for `stack_idx`, reusing an existing one if
    /// present (so all closures that capture the same slot share one cell).
    fn capture_upvalue(&mut self, stack_idx: usize) -> Upvalue {
        if let Some(uv) = self
            .open_upvalues
            .iter()
            .find(|uv| matches!(*uv.0.borrow(), UpvalueState::Open(i) if i == stack_idx))
        {
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
            if let UpvalueState::Open(idx) = *state
                && idx >= first_slot
            {
                let val = self.stack[idx].clone();
                *state = UpvalueState::Closed(val);
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
            ip: 0,
            base,
            block: None,
            is_block_caller: false,
            is_native_block: true,
            rescues: vec![],
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
            VmValue::List(r) => {
                let r = *r;
                match name {
                    "each" => {
                        let items: Vec<VmValue> = self.get_list(r).clone();
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
                        let items: Vec<VmValue> = self.get_list(r).clone();
                        let mut out = Vec::with_capacity(items.len());
                        for item in items {
                            match self.call_block(&blk, vec![item]) {
                                Err(VmError::Next(v)) => out.push(v),
                                Err(VmError::Break(v)) => {
                                    out.push(v);
                                    break;
                                }
                                Err(e) => return Err(e),
                                Ok(v) => out.push(v),
                            }
                        }
                        Ok(self.alloc_list(out))
                    }
                    "select" => {
                        let items: Vec<VmValue> = self.get_list(r).clone();
                        let mut out = Vec::new();
                        for item in items {
                            match self.call_block(&blk, vec![item.clone()]) {
                                Err(VmError::Break(_)) => break,
                                Err(e) => return Err(e),
                                Ok(v) if !is_falsy(&v) => out.push(item),
                                Ok(_) => {}
                            }
                        }
                        Ok(self.alloc_list(out))
                    }
                    "reject" => {
                        let items: Vec<VmValue> = self.get_list(r).clone();
                        let mut out = Vec::new();
                        for item in items {
                            match self.call_block(&blk, vec![item.clone()]) {
                                Err(VmError::Break(_)) => break,
                                Err(e) => return Err(e),
                                Ok(v) if is_falsy(&v) => out.push(item),
                                Ok(_) => {}
                            }
                        }
                        Ok(self.alloc_list(out))
                    }
                    "any?" => {
                        let items: Vec<VmValue> = self.get_list(r).clone();
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
                        let items: Vec<VmValue> = self.get_list(r).clone();
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
                        let items: Vec<VmValue> = self.get_list(r).clone();
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
                    "reduce" => {
                        let items: Vec<VmValue> = self.get_list(r).clone();
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
                        let items: Vec<VmValue> = self.get_list(r).clone();
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
                        message: format!("List has no block method '{}'", name),
                        line,
                    }),
                }
            }

            VmValue::Range { from, to } => {
                let (from, to) = (*from, *to);
                match name {
                    "each" => {
                        let mut i = from;
                        while i < to {
                            match self.call_block(&blk, vec![VmValue::Int(i)]) {
                                Err(VmError::Next(_)) => {
                                    i += 1;
                                    continue;
                                }
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
                        Ok(self.alloc_list(out))
                    }
                    _ => Err(VmError::TypeError {
                        message: format!("Range has no block method '{}'", name),
                        line,
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
                        _ => {
                            return Err(VmError::TypeError {
                                message: "upto expects an Int".into(),
                                line,
                            });
                        }
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
                    message: format!("Int has no block method '{}'", name),
                    line,
                }),
            },

            VmValue::Map(r) => {
                let r = *r;
                match name {
                    "each" => {
                        let pairs: Vec<(String, VmValue)> = self
                            .get_map(r)
                            .iter()
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
                        let pairs: Vec<(String, VmValue)> = self
                            .get_map(r)
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let mut out = Vec::with_capacity(pairs.len());
                        for (k, v) in pairs {
                            out.push(self.call_block(&blk, vec![VmValue::Str(k), v])?);
                        }
                        Ok(self.alloc_list(out))
                    }
                    _ => Err(VmError::TypeError {
                        message: format!("Map has no block method '{}'", name),
                        line,
                    }),
                }
            }

            VmValue::Set(r) => {
                let r = *r;
                match name {
                    "each" => {
                        let items: Vec<VmValue> = self.get_set(r).clone();
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
                    _ => Err(VmError::TypeError {
                        message: format!("Set has no block method '{}'", name),
                        line,
                    }),
                }
            }

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

    // ── Test runner ───────────────────────────────────────────────────────────

    /// Return all subclasses of `Test` (excluding `Test` itself) together with
    /// the list of their test methods (names starting with `test_`).
    /// Each entry is `(class_name, Vec<(test_label, VmMethod)>)` where
    /// `test_label` is the method name with the `test_` prefix stripped.
    pub fn collect_test_classes(&self) -> Vec<(String, Vec<(String, VmMethod)>)> {
        let mut result = Vec::new();
        let mut names: Vec<&String> = self.classes.keys().collect();
        names.sort();
        for class_name in names {
            if class_name == "Test" {
                continue;
            }
            if !vm_is_subclass(&self.classes, class_name.as_str(), "Test") {
                continue;
            }
            let entry = &self.classes[class_name];
            let mut tests: Vec<(String, VmMethod)> = entry
                .methods
                .iter()
                .filter(|(name, _)| name.starts_with("test_"))
                .map(|(name, method)| {
                    (name.trim_start_matches("test_").to_string(), method.clone())
                })
                .collect();
            tests.sort_by(|a, b| a.0.cmp(&b.0));
            if !tests.is_empty() {
                result.push((class_name.clone(), tests));
            }
        }
        result
    }

    /// Call a single method on an instance without disturbing the existing
    /// stack.  Returns `Err(message)` if the method raises or there is a VM
    /// error.
    fn call_method_on_instance(
        &mut self,
        instance: VmValue,
        method: &VmMethod,
    ) -> Result<(), String> {
        let min_depth = self.frames.len();
        let base = self.stack.len();
        self.stack.push(instance);
        self.frames.push(CallFrame {
            function: method.function.clone(),
            upvalues: method.upvalues.clone(),
            ip: 0,
            base,
            block: None,
            is_block_caller: false,
            is_native_block: false,
            rescues: vec![],
            class_name: Some(method.defined_in.clone()),
        });
        let result = self.run_inner(min_depth);
        self.stack.truncate(base);
        self.frames.truncate(min_depth);
        result.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Run a single test: build a fresh instance of `class_name`, call
    /// `setup`, run `test_method`, call `teardown`.  Returns `Ok(())` on
    /// success or `Err(message)` if any step raises.
    pub fn run_single_test(
        &mut self,
        class_name: &str,
        test_method: &VmMethod,
    ) -> Result<(), String> {
        let entry = self
            .classes
            .get(class_name)
            .ok_or_else(|| format!("class '{}' not found", class_name))?;

        let ancestor_chain = Rc::new(entry.ancestors.clone());
        let fields_map: HashMap<String, VmValue> = entry
            .fields
            .iter()
            .map(|(name, val)| (name.clone(), val.clone()))
            .collect();
        let methods = entry.methods.clone();
        let fields = self.alloc_fields(fields_map);
        let instance = VmValue::Instance {
            class_name: class_name.to_string(),
            ancestor_chain,
            fields,
            methods: methods.clone(),
        };

        // Call setup if defined and not the base no-op from Test itself.
        if let Some(setup) = methods.get("setup") {
            self.call_method_on_instance(instance.clone(), setup)?;
        }

        // Run the test.
        self.call_method_on_instance(instance.clone(), test_method)?;

        // Call teardown if defined.
        if let Some(teardown) = methods.get("teardown") {
            self.call_method_on_instance(instance.clone(), teardown)?;
        }

        Ok(())
    }
}

fn starting_class_name_for_is_a(recv: &VmValue) -> Option<String> {
    match recv {
        VmValue::Instance { class_name, .. } => Some(class_name.clone()),
        _ => primitive_class_name(recv).map(|s: &str| s.to_string()),
    }
}

/// True if `start` is `target`, or `target` appears later in `start`'s ancestor chain
/// (superclass or included module).
fn vm_is_subclass(classes: &HashMap<String, ClassEntry>, start: &str, target: &str) -> bool {
    if start == target {
        return true;
    }
    let Some(entry) = classes.get(start) else {
        return false;
    };
    let chain = &entry.ancestors;
    let Some(idx_start) = chain.iter().position(|a| a == start) else {
        return false;
    };
    chain
        .iter()
        .position(|a| a == target)
        .is_some_and(|idx_target| idx_target > idx_start)
}

fn invoke_is_a(
    heap: &GcHeap<HeapObject>,
    classes: &HashMap<String, ClassEntry>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let target = match args.first() {
        Some(VmValue::Class { name, .. }) => name.clone(),
        Some(VmValue::ClassObj(r)) => match heap.get(*r) {
            HeapObject::ClassObject { name, .. } => name.clone(),
            _ => {
                return Err(VmError::TypeError {
                    message: "is_a? requires a class argument".into(),
                    line,
                });
            }
        },
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
    Ok(VmValue::Bool(vm_is_subclass(classes, start.as_str(), &target)))
}
