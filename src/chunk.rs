use std::rc::Rc;

/// Describes how a closure captures a variable from an enclosing scope.
#[derive(Debug, Clone)]
pub struct UpvalueDef {
    /// If true, the captured variable is a local in the immediately enclosing
    /// function's stack frame.  If false, it is itself an upvalue of the
    /// enclosing function (i.e. captured transitively).
    pub is_local: bool,
    pub index:    usize,
}

/// A compiled function: its own bytecode chunk, name, arity, and the upvalue
/// descriptors needed to build a closure at runtime.
#[derive(Debug)]
pub struct Function {
    pub name:         String,
    pub arity:        usize,
    pub chunk:        Chunk,
    pub upvalue_defs: Vec<UpvalueDef>,
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

    // Comparison
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,

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

    // Stack manipulation
    Pop,
    Return,

    // Literals
    True,
    False,
    Nil,
}

/// A runtime constant — the only values the compiler can embed directly.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Str(String),
    Function(Rc<Function>),
}

impl std::fmt::Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Constant::Int(n)         => write!(f, "{}", n),
            Constant::Float(n)       => write!(f, "{}", n),
            Constant::Str(s)         => write!(f, "{:?}", s),
            Constant::Function(func) => write!(f, "<fn {}>", func.name),
        }
    }
}

/// A sequence of instructions plus the constants they reference.
#[derive(Debug, Default)]
pub struct Chunk {
    pub code:      Vec<OpCode>,
    pub constants: Vec<Constant>,
    /// Source line number parallel to `code`, for error messages.
    pub lines:     Vec<u32>,
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
            OpCode::Jump(_)        => self.code[jump_idx] = OpCode::Jump(offset),
            OpCode::JumpIfFalse(_) => self.code[jump_idx] = OpCode::JumpIfFalse(offset),
            _ => panic!("patch_jump called on non-jump instruction"),
        }
    }

    /// Add a constant and return its index.
    pub fn add_constant(&mut self, value: Constant) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    /// Print a human-readable listing of the chunk.
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
                OpCode::Constant(idx)     => println!("CONSTANT       {:4}  ({})", idx, self.constants[*idx]),
                OpCode::Closure(idx)      => println!("CLOSURE        {:4}  ({})", idx, self.constants[*idx]),
                OpCode::GetLocal(slot)    => println!("GET_LOCAL      {:4}", slot),
                OpCode::SetLocal(slot)    => println!("SET_LOCAL      {:4}", slot),
                OpCode::GetUpvalue(idx)   => println!("GET_UPVALUE    {:4}", idx),
                OpCode::SetUpvalue(idx)   => println!("SET_UPVALUE    {:4}", idx),
                OpCode::Jump(off)         => println!("JUMP           {:4}", off),
                OpCode::JumpIfFalse(off)  => println!("JUMP_IF_FALSE  {:4}", off),
                OpCode::Loop(off)         => println!("LOOP           {:4}", off),
                OpCode::Call(argc)        => println!("CALL           {:4}", argc),
                other                     => println!("{:?}", other),
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
