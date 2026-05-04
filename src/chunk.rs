use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeType {
    Named(String),
    Literal(crate::value::Value),
    Union(Vec<RuntimeType>),
}

/// Describes how a closure captures a variable from an enclosing scope.
#[derive(Debug, Clone)]
pub struct UpvalueDef {
    /// If true, the captured variable is a local in the immediately enclosing
    /// function's stack frame.  If false, it is itself an upvalue of the
    /// enclosing function (i.e. captured transitively).
    pub is_local: bool,
    pub index: usize,
}

/// A compiled function: its own bytecode chunk, name, arity, and the upvalue
/// descriptors needed to build a closure at runtime.
#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    pub chunk: Chunk,
    pub upvalue_defs: Vec<UpvalueDef>,
    /// Return type annotation, if one was present in the source (`-> TypeName`).
    /// When `Some`, the VM checks the actual return value at runtime.
    pub return_type: Option<RuntimeType>,
}

/// Two `Function` values are equal only if they are the exact same allocation.
impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

/// A single VM instruction.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum OpCode {
    // Push the constant at `constants[index]` onto the stack.
    Constant(usize),

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Comparison
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    Shl,
    Shr,

    // Unary
    Negate,
    Not,

    // Local variables  (slot = index into the stack frame)
    GetLocal(usize),
    SetLocal(usize),

    // Captured variables (upvalues)
    GetUpvalue(usize),
    SetUpvalue(usize),
    /// Close the open upvalue at the top of the stack, then pop the slot.
    CloseUpvalue,

    // Jumps (offset = number of instructions to skip forward from the next ip)
    Jump(usize),
    /// Pop the top of stack; if falsy, jump forward by `offset`.
    JumpIfFalse(usize),
    /// Jump backward: subtract `offset` from ip (used for loops).
    Loop(usize),

    // Functions & closures
    /// Like `Constant`, but wraps the function in a closure capturing upvalues
    /// according to the function's `upvalue_defs`.
    Closure(usize),
    /// Call the callable sitting `arg_count` slots below the top of the stack.
    Call(usize),

    // Short-circuit boolean ops: peek at TOS; if the condition holds, jump
    // without popping (leaves the short-circuit value as the result); if the
    // condition does NOT hold, pop TOS and fall through to evaluate the RHS.
    /// Used for `and`: if TOS is falsy, jump keeping TOS; else pop and continue.
    JumpIfFalseKeep(usize),
    /// Used for `or`: if TOS is truthy, jump keeping TOS; else pop and continue.
    JumpIfTrueKeep(usize),

    // String interpolation
    /// Pop `n` values off the stack, convert each to its Display string,
    /// concatenate them in order, and push the resulting Str.
    BuildString(usize),

    // Collections
    /// Pop `n` values and push a List containing them in order.
    BuildList(usize),
    /// Pop `n` (key, value) pairs — keys are Str on the stack — and push a Map.
    BuildMap(usize),
    /// Pop two Int values (from, to) and push a Range.
    BuildRange,

    // Indexing
    /// Pop index then object; push the element at that index.
    Index,
    /// Pop value, index, then object; set the element; push value.
    IndexSet,

    // Classes
    /// Pops N method closures (N = ClassDesc.method_names.len()) and the
    /// constant at `const_idx` provides the ClassDesc; pushes a Class value.
    DefClass(usize),
    /// Class construction: the stack has [class, name0, val0, …, nameN, valN].
    /// Pops 2*n+1 values, creates an Instance with the supplied field values.
    NewInstance(usize),
    /// Pop object, look up field `const_idx` (a Str constant) in instance
    /// fields, push the value (or Nil if absent).
    GetField(usize),
    /// Like GetField but returns Nil (instead of erroring) when the object is Nil.
    GetFieldSafe(usize),
    /// Stack: [object, value]; set instance field `const_idx`; push value.
    SetField(usize),
    /// Method call: receiver is at stack[len - arg_count - 1].
    /// `name_idx` is a Str constant; looks up the method, pushes a new frame.
    Invoke(usize, usize),
    /// Like Invoke but dispatches to the superclass's method.
    /// Uses the current frame's class name to locate the superclass.
    SuperInvoke(usize, usize),
    /// Push `self` (slot 0) — emitted for `SelfExpr` inside methods.
    GetSelf,
    /// Resolve a class constant by name using the lexically enclosing classes
    /// recorded at compile time, then fall back to `GetGlobal` semantics.
    /// Operand is `Constant::LexicalClassScope`.
    GetLexicalConstant(usize),

    // Blocks
    /// Like `Call(n)` but also passes the block closure sitting one slot
    /// above the function value (pushed there by the compiler before args).
    /// Stack layout: [..., fn, block, arg0, …, argN-1]
    CallWithBlock(usize),
    /// Like `Invoke(name, n)` but passes an additional block argument.
    /// Stack: [..., receiver, block, arg0, …, argN-1]
    InvokeWithBlock(usize, usize),
    /// Call the block that was passed to the current function.
    /// The block lives in a dedicated upvalue slot set up at call time.
    Yield(usize),

    // Exception-like control flow
    /// Pop TOS and signal a raise: walks up frames looking for BeginRescue.
    Raise,
    /// Pop TOS and unwind to the nearest block-caller frame; push the value.
    Break,
    /// Pop TOS and immediately return from the current (block) frame with it.
    Next,
    /// Register a rescue handler for the current frame.
    /// `handler_offset`: instruction offset from HERE to the rescue body.
    /// `rescue_var_slot`: local slot index for the caught value (usize::MAX = none).
    BeginRescue {
        handler_offset: usize,
        rescue_var_slot: usize,
    },
    /// Remove the most recently registered rescue handler (normal exit from body).
    PopRescue,

    // Native helpers used by pre-built stdlib functions
    /// Pop a List/Map/Str/Range and push its size as Int.
    Len,
    /// Pop a Map and push a List of its string keys (sorted).
    MapKeys,
    /// Pop a Range and push its `from` field as Int.
    RangeFrom,
    /// Pop a Range and push its `to` field as Int.
    RangeTo,

    // Output
    /// Pop TOS, print it with a newline, push Nil.
    Print,

    // Module loading
    /// Load, compile, and execute a relative file in the current scope.
    /// `path_idx` is a Str constant holding the import path.
    Import(usize),

    // Global variables (used by the REPL to persist state across calls)
    /// Push the value of a global variable onto the stack.
    /// `idx` is an index into the constants table pointing to a `Constant::Str` name.
    GetGlobal(usize),
    /// Store TOS into a global variable (peek — does not pop).
    /// `idx` is an index into the constants table pointing to a `Constant::Str` name.
    SetGlobal(usize),

    // Stack manipulation
    Pop,
    Return,
    /// Explicit `return` statement inside a block called by a native method.
    /// Behaves like `Return` in normal frames; raises `VmError::Return` in
    /// `is_native_block` frames so the dispatcher can perform a non-local return.
    NonLocalReturn,

    // Literals
    True,
    False,
    Nil,

    // Pattern matching helpers
    /// Pop value; push Bool — true if the value is an instance of the named class/type.
    IsA(String),
    /// Pop a List; push its length as Int.  Errors if the value is not a List.
    ListLen,
}

/// A runtime constant — the only values the compiler can embed directly.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Str(String),
    Function(Rc<Function>),
    /// Static descriptor for a class: its name, optional superclass name,
    /// field names (in declaration order), and method names (in the same
    /// order as the closures that will be popped off the stack by `DefClass`).
    ClassDesc {
        name: String,
        /// Static superclass name — used when the superclass is a plain `Variable`.
        superclass: Option<String>,
        /// When true, the superclass value is on the stack (TOS) and must be popped by DefClass.
        superclass_dynamic: bool,
        field_names: Vec<String>,
        field_defaults: Vec<Option<Constant>>,
        method_names: Vec<String>,
        private_methods: Vec<String>,
        class_method_names: Vec<String>,
        /// Names of nested classes; matched 1-to-1 with class values pushed after instance methods.
        nested_class_names: Vec<String>,
    },
    /// Enclosing class names from outer to inner for `GetLexicalConstant`; `name_idx` points at
    /// a `Constant::Str` holding the constant name to resolve.
    LexicalClassScope {
        enclosing_classes: Vec<String>,
        name_idx: usize,
    },
}

impl std::fmt::Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Constant::Int(n) => write!(f, "{}", n),
            Constant::Float(n) => write!(f, "{}", n),
            Constant::Str(s) => write!(f, "{:?}", s),
            Constant::Function(func) => write!(f, "<fn {}>", func.name),
            Constant::ClassDesc {
                name,
                superclass: Some(s),
                ..
            } => write!(f, "<class {} extends {}>", name, s),
            Constant::ClassDesc {
                name,
                superclass_dynamic: true,
                ..
            } => write!(f, "<class {} extends (dynamic)>", name),
            Constant::ClassDesc { name, .. } => write!(f, "<class {}>", name),
            Constant::LexicalClassScope { .. } => write!(f, "<lexical class scope>"),
        }
    }
}

/// A sequence of instructions plus the constants they reference.
#[derive(Debug, Default)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Constant>,
    /// Source line number parallel to `code`, for error messages.
    pub lines: Vec<u32>,
}

impl Chunk {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, op: OpCode, line: u32) {
        self.code.push(op);
        self.lines.push(line);
    }

    /// Overwrite a previously-emitted jump instruction with the correct offset.
    /// Call this after the jump target has been emitted.
    pub fn patch_jump(&mut self, jump_idx: usize) {
        let offset = self.code.len() - jump_idx - 1;
        match &self.code[jump_idx] {
            OpCode::Jump(_) => self.code[jump_idx] = OpCode::Jump(offset),
            OpCode::JumpIfFalse(_) => self.code[jump_idx] = OpCode::JumpIfFalse(offset),
            OpCode::JumpIfFalseKeep(_) => self.code[jump_idx] = OpCode::JumpIfFalseKeep(offset),
            OpCode::JumpIfTrueKeep(_) => self.code[jump_idx] = OpCode::JumpIfTrueKeep(offset),
            _ => panic!("patch_jump called on non-jump instruction"),
        }
    }

    /// Patch the handler_offset of a previously-emitted `BeginRescue`.
    /// Call this when the rescue handler body's start position is known.
    pub fn patch_rescue(&mut self, begin_idx: usize) {
        let offset = self.code.len() - begin_idx - 1;
        match &self.code[begin_idx] {
            OpCode::BeginRescue {
                rescue_var_slot, ..
            } => {
                let slot = *rescue_var_slot;
                self.code[begin_idx] = OpCode::BeginRescue {
                    handler_offset: offset,
                    rescue_var_slot: slot,
                };
            }
            _ => panic!("patch_rescue called on non-BeginRescue instruction"),
        }
    }

    /// Add a constant and return its index.
    pub fn add_constant(&mut self, value: Constant) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    /// Print a human-readable listing of the chunk.
    #[allow(dead_code)]
    pub fn disassemble(&self, name: &str) {
        println!("=== {} ===", name);
        for (offset, op) in self.code.iter().enumerate() {
            let line = self.lines[offset];
            let line_str = if offset > 0 && self.lines[offset - 1] == line {
                "   |".to_string()
            } else {
                format!("{:4}", line)
            };
            print!("{:04}  {}  ", offset, line_str);
            match op {
                OpCode::Constant(idx) => {
                    println!("CONSTANT       {:4}  ({})", idx, self.constants[*idx])
                }
                OpCode::Closure(idx) => {
                    println!("CLOSURE        {:4}  ({})", idx, self.constants[*idx])
                }
                OpCode::GetLocal(slot) => println!("GET_LOCAL      {:4}", slot),
                OpCode::SetLocal(slot) => println!("SET_LOCAL      {:4}", slot),
                OpCode::GetUpvalue(idx) => println!("GET_UPVALUE    {:4}", idx),
                OpCode::SetUpvalue(idx) => println!("SET_UPVALUE    {:4}", idx),
                OpCode::Jump(off) => println!("JUMP           {:4}", off),
                OpCode::JumpIfFalse(off) => println!("JUMP_IF_FALSE  {:4}", off),
                OpCode::Loop(off) => println!("LOOP           {:4}", off),
                OpCode::Call(argc) => println!("CALL                {:4}", argc),
                OpCode::BuildString(n) => println!("BUILD_STRING        {:4}", n),
                OpCode::BuildList(n) => println!("BUILD_LIST          {:4}", n),
                OpCode::BuildMap(n) => println!("BUILD_MAP           {:4}", n),
                OpCode::DefClass(idx) => {
                    println!("DEF_CLASS           {:4}  ({})", idx, self.constants[*idx])
                }
                OpCode::NewInstance(n) => println!("NEW_INSTANCE        {:4}", n),
                OpCode::GetField(idx) => {
                    println!("GET_FIELD           {:4}  ({})", idx, self.constants[*idx])
                }
                OpCode::GetFieldSafe(idx) => {
                    println!("GET_FIELD_SAFE      {:4}  ({})", idx, self.constants[*idx])
                }
                OpCode::SetField(idx) => {
                    println!("SET_FIELD           {:4}  ({})", idx, self.constants[*idx])
                }
                OpCode::Invoke(n, argc) => println!("INVOKE              {:4}  argc={}", n, argc),
                OpCode::SuperInvoke(n, argc) => {
                    println!("SUPER_INVOKE        {:4}  argc={}", n, argc)
                }
                OpCode::JumpIfFalseKeep(off) => println!("JUMP_IF_FALSE_KEEP  {:4}", off),
                OpCode::JumpIfTrueKeep(off) => println!("JUMP_IF_TRUE_KEEP   {:4}", off),
                OpCode::GetGlobal(idx) => {
                    println!("GET_GLOBAL          {:4}  ({})", idx, self.constants[*idx])
                }
                OpCode::SetGlobal(idx) => {
                    println!("SET_GLOBAL          {:4}  ({})", idx, self.constants[*idx])
                }
                OpCode::Import(idx) => {
                    println!("IMPORT              {:4}  ({})", idx, self.constants[*idx])
                }
                other => println!("{:?}", other),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_round_trip() {
        let mut chunk = Chunk::new();
        let idx = chunk.add_constant(Constant::Int(42));
        chunk.write(OpCode::Constant(idx), 1);
        chunk.write(OpCode::Return, 1);

        assert_eq!(chunk.code.len(), 2);
        assert_eq!(chunk.constants[idx], Constant::Int(42));
        assert_eq!(chunk.code[0], OpCode::Constant(0));
        assert_eq!(chunk.code[1], OpCode::Return);
    }

    #[test]
    fn disassemble_does_not_panic() {
        let mut chunk = Chunk::new();
        let i = chunk.add_constant(Constant::Float(3.14));
        chunk.write(OpCode::Constant(i), 1);
        chunk.write(OpCode::Negate, 1);
        chunk.write(OpCode::Return, 1);
        chunk.disassemble("test chunk"); // should not panic
    }
}
