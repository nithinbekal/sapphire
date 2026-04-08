use std::fmt;
use std::rc::Rc;
use crate::ast::{Block, Expr, Stmt, StringPart, MethodDef};
use crate::chunk::{Chunk, Constant, Function, OpCode, UpvalueDef};
use crate::token::TokenKind;
use crate::value::Value;

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub struct CompileError {
    pub message: String,
    pub line:    u32,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[line {}] compile error: {}", self.line, self.message)
    }
}

// ── Per-function compiler state ───────────────────────────────────────────────

struct LocalInfo {
    name:     String,
    /// True once another closure has captured this local as an upvalue.
    captured: bool,
}

struct UpvalueInfo {
    is_local: bool,   // local in the immediately enclosing scope
    index:    usize,
}

struct FunctionState {
    chunk:        Chunk,
    current_line: u32,
    locals:       Vec<LocalInfo>,
    upvalue_defs: Vec<UpvalueInfo>,
    name:         String,
    arity:        usize,
}

impl FunctionState {
    fn new(name: &str, arity: usize, line: u32) -> Self {
        FunctionState {
            chunk:        Chunk::new(),
            current_line: line,
            locals:       Vec::new(),
            upvalue_defs: Vec::new(),
            name:         name.to_string(),
            arity,
        }
    }
}

// ── Compiler ──────────────────────────────────────────────────────────────────

/// A stack of `FunctionState`s, one per nesting level.  The top of the stack
/// is the function currently being compiled.
struct Compiler {
    states: Vec<FunctionState>,
}

impl Compiler {
    fn new() -> Self {
        Compiler { states: Vec::new() }
    }

    // ── Public entry points ───────────────────────────────────────────────────

    /// Compile a top-level program (arity 0, anonymous).
    pub fn compile(stmts: &[Stmt]) -> Result<Rc<Function>, CompileError> {
        let mut c = Compiler::new();
        c.push_fn("", 0, 0);
        c.compile_body(stmts)?;
        Ok(c.pop_fn())
    }

    // ── Function stack ────────────────────────────────────────────────────────

    fn push_fn(&mut self, name: &str, arity: usize, line: u32) {
        self.states.push(FunctionState::new(name, arity, line));
    }

    fn pop_fn(&mut self) -> Rc<Function> {
        let state = self.states.pop().unwrap();
        Rc::new(Function {
            name:  state.name,
            arity: state.arity,
            chunk: state.chunk,
            upvalue_defs: state.upvalue_defs
                .into_iter()
                .map(|uv| UpvalueDef { is_local: uv.is_local, index: uv.index })
                .collect(),
        })
    }

    fn state(&self) -> &FunctionState {
        self.states.last().unwrap()
    }

    fn state_mut(&mut self) -> &mut FunctionState {
        self.states.last_mut().unwrap()
    }

    // ── Body compilation (shared between top-level and functions) ─────────────

    /// Compile a list of statements, treating the last expression as an
    /// implicit return value (Ruby-style).
    fn compile_body(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        let (last, rest) = match stmts.split_last() {
            Some(pair) => pair,
            None => {
                self.emit(OpCode::Nil);
                self.emit(OpCode::Return);
                return Ok(());
            }
        };
        self.stmts(rest)?;
        match last {
            Stmt::Expression(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Return);
            }
            Stmt::Print(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Print);
                self.emit(OpCode::Return);
            }
            Stmt::Class { .. } => {
                self.stmt(last)?;
                self.emit(OpCode::Return);
            }
            Stmt::Function { name, .. } => {
                self.stmt(last)?;
                let idx = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                self.emit(OpCode::Constant(idx));
                self.emit(OpCode::Return);
            }
            other => {
                self.stmt(other)?;
                self.emit(OpCode::Nil);
                self.emit(OpCode::Return);
            }
        }
        Ok(())
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Expression(expr) => {
                let is_new_local = self.is_new_local_assign(expr);
                self.expr(expr)?;
                // A defining assignment reserves a stack slot — don't pop it.
                if !is_new_local {
                    self.emit(OpCode::Pop);
                }
            }

            Stmt::Return(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Return);
            }

            Stmt::Function { name, params, body, .. } => {
                let line  = self.state().current_line;
                let arity = params.len();

                self.push_fn(name, arity, line);
                // Slot 0 = the function itself — enables direct recursion by name.
                self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
                for p in params {
                    self.state_mut().locals.push(LocalInfo { name: p.name.clone(), captured: false });
                }
                self.compile_body(body)?;
                let func = self.pop_fn();

                let idx = self.state_mut().chunk.add_constant(Constant::Function(func));
                self.emit(OpCode::Closure(idx));
                // The closure value on the stack becomes this local's slot.
                self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
            }

            Stmt::If { condition, then_branch, else_branch } => {
                self.expr(condition)?;
                let jif = self.emit_jump(OpCode::JumpIfFalse(0));

                self.stmts(then_branch)?;

                match else_branch {
                    Some(else_stmts) => {
                        let jump = self.emit_jump(OpCode::Jump(0));
                        self.patch_jump(jif);
                        self.stmts(else_stmts)?;
                        self.patch_jump(jump);
                    }
                    None => {
                        self.patch_jump(jif);
                    }
                }
            }

            Stmt::While { condition, body } => {
                let loop_start = self.state().chunk.code.len();

                self.expr(condition)?;
                let exit_jump = self.emit_jump(OpCode::JumpIfFalse(0));

                self.stmts(body)?;
                self.emit_loop(loop_start);

                self.patch_jump(exit_jump);
            }

            Stmt::Print(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Print);
                self.emit(OpCode::Pop);
            }

            Stmt::Class { name, superclass, fields, methods } => {
                self.compile_class(name, superclass.as_deref(), fields, methods)?;
            }

            Stmt::Raise(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Raise);
            }

            Stmt::Break(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Break);
            }

            Stmt::Next(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Next);
            }

            Stmt::Begin { body, rescue_var, rescue_body, else_body } => {
                self.compile_begin(body, rescue_var, rescue_body, else_body)?;
            }

            Stmt::MultiAssign { names, values } => {
                if names.len() != values.len() {
                    return Err(self.error(format!(
                        "expected {} value(s), got {}", names.len(), values.len()
                    )));
                }
                // Resolve each name as an existing local/upvalue, or pre-allocate
                // a new nil slot.  Using Ok(slot) for locals and Err(idx) for upvalues.
                let mut targets: Vec<Result<usize, usize>> = Vec::with_capacity(names.len());
                let depth = self.states.len() - 1;
                for name in names {
                    if let Some(slot) = self.resolve_local(depth, name) {
                        targets.push(Ok(slot));
                    } else if let Some(idx) = self.resolve_upvalue(depth, name) {
                        targets.push(Err(idx));
                    } else {
                        // New variable: push nil as its stack slot, then register.
                        let slot = self.state().locals.len();
                        self.emit(OpCode::Nil);
                        self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
                        targets.push(Ok(slot));
                    }
                }
                // Evaluate all RHS before any assignment (enables `a, b = b, a`).
                for val_expr in values {
                    self.expr(val_expr)?;
                }
                // Assign top-of-stack first so RHS values go to the right variables.
                for t in targets.into_iter().rev() {
                    match t {
                        Ok(slot) => {
                            self.emit(OpCode::SetLocal(slot));
                            self.emit(OpCode::Pop);
                        }
                        Err(idx) => {
                            self.emit(OpCode::SetUpvalue(idx));
                            self.emit(OpCode::Pop);
                        }
                    }
                }
            }

            #[allow(unreachable_patterns)]
            other => {
                return Err(self.error(format!(
                    "statement not yet supported by compiler: {:?}",
                    std::mem::discriminant(other)
                )));
            }
        }
        Ok(())
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    fn expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Literal(val) => self.literal(val),

            Expr::Grouping(inner) => self.expr(inner),

            Expr::Unary { op, right } => {
                self.state_mut().current_line = op.line as u32;
                self.expr(right)?;
                match &op.kind {
                    TokenKind::Minus => self.emit(OpCode::Negate),
                    TokenKind::Bang  => self.emit(OpCode::Not),
                    other => return Err(self.error(format!("unknown unary op: {:?}", other))),
                }
                Ok(())
            }

            Expr::Binary { left, op, right } => {
                self.state_mut().current_line = op.line as u32;
                // Short-circuit operators: evaluate only the left side first.
                match &op.kind {
                    TokenKind::AmpAmp => {
                        // `a and b`: if a is falsy keep a as result; else result is b.
                        self.expr(left)?;
                        let jump = self.emit_jump(OpCode::JumpIfFalseKeep(0));
                        self.expr(right)?;
                        self.patch_jump(jump);
                        return Ok(());
                    }
                    TokenKind::PipePipe => {
                        // `a or b`: if a is truthy keep a as result; else result is b.
                        self.expr(left)?;
                        let jump = self.emit_jump(OpCode::JumpIfTrueKeep(0));
                        self.expr(right)?;
                        self.patch_jump(jump);
                        return Ok(());
                    }
                    _ => {}
                }
                self.expr(left)?;
                self.expr(right)?;
                match &op.kind {
                    TokenKind::Plus      => self.emit(OpCode::Add),
                    TokenKind::Minus     => self.emit(OpCode::Sub),
                    TokenKind::Star      => self.emit(OpCode::Mul),
                    TokenKind::Slash     => self.emit(OpCode::Div),
                    TokenKind::EqEq      => self.emit(OpCode::Equal),
                    TokenKind::BangEq    => self.emit(OpCode::NotEqual),
                    TokenKind::Less      => self.emit(OpCode::Less),
                    TokenKind::LessEq    => self.emit(OpCode::LessEqual),
                    TokenKind::Greater   => self.emit(OpCode::Greater),
                    TokenKind::GreaterEq => self.emit(OpCode::GreaterEqual),
                    other => return Err(self.error(format!("unknown binary op: {:?}", other))),
                }
                Ok(())
            }

            Expr::Variable(name) => {
                let depth = self.states.len() - 1;
                if let Some(slot) = self.resolve_local(depth, name) {
                    self.emit(OpCode::GetLocal(slot));
                } else if let Some(idx) = self.resolve_upvalue(depth, name) {
                    self.emit(OpCode::GetUpvalue(idx));
                } else {
                    return Err(self.error(format!("undefined variable '{}'", name)));
                }
                Ok(())
            }

            Expr::Assign { name, value } => {
                self.expr(value)?;
                let depth = self.states.len() - 1;
                if let Some(slot) = self.resolve_local(depth, name) {
                    self.emit(OpCode::SetLocal(slot));
                } else if let Some(idx) = self.resolve_upvalue(depth, name) {
                    self.emit(OpCode::SetUpvalue(idx));
                } else {
                    // First use of this name: allocate a new stack slot.
                    self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
                }
                Ok(())
            }

            Expr::SelfExpr => {
                self.emit(OpCode::GetSelf);
                Ok(())
            }

            Expr::Call { callee, args, block } => {
                // Method call: `obj.method(args)` or `obj.method(args) { |x| ... }`
                if let Expr::Get { object, name } = callee.as_ref() {
                    if name == "new" && block.is_none() {
                        // Class construction: `ClassName.new(field: val, ...)`
                        self.expr(object)?;
                        for arg in args {
                            let arg_name = arg.name.clone().unwrap_or_default();
                            let ni = self.state_mut().chunk.add_constant(Constant::Str(arg_name));
                            self.emit(OpCode::Constant(ni));
                            self.expr(&arg.value)?;
                        }
                        self.emit(OpCode::NewInstance(args.len()));
                    } else if let Some(blk) = block {
                        self.expr(object)?;
                        for arg in args {
                            self.expr(&arg.value)?;
                        }
                        self.compile_block(blk)?;
                        let ni = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                        self.emit(OpCode::InvokeWithBlock(ni, args.len()));
                    } else {
                        // Regular method invocation
                        self.expr(object)?;
                        for arg in args {
                            self.expr(&arg.value)?;
                        }
                        let ni = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                        self.emit(OpCode::Invoke(ni, args.len()));
                    }
                    return Ok(());
                }
                // Implicit self dispatch: if the callee is a bare name that can't be
                // resolved as a local/upvalue, but `self` is in scope (we're inside a
                // method body), rewrite as `self.name(args)`.
                if let Expr::Variable(name) = callee.as_ref() {
                    let depth = self.states.len() - 1;
                    let is_unresolved = self.resolve_local(depth, name).is_none()
                        && self.resolve_upvalue(depth, name).is_none();
                    let self_in_scope = self.resolve_local(depth, "self").is_some();
                    if is_unresolved && self_in_scope {
                        self.emit(OpCode::GetSelf);
                        let arg_count = args.len();
                        for arg in args {
                            self.expr(&arg.value)?;
                        }
                        let ni = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                        if let Some(blk) = block {
                            self.compile_block(blk)?;
                            self.emit(OpCode::InvokeWithBlock(ni, arg_count));
                        } else {
                            self.emit(OpCode::Invoke(ni, arg_count));
                        }
                        return Ok(());
                    }
                }
                // Plain function call (with or without block)
                self.expr(callee)?;
                let arg_count = args.len();
                for arg in args {
                    self.expr(&arg.value)?;
                }
                if let Some(blk) = block {
                    self.compile_block(blk)?;
                    self.emit(OpCode::CallWithBlock(arg_count));
                } else {
                    self.emit(OpCode::Call(arg_count));
                }
                Ok(())
            }

            Expr::Yield { args } => {
                let n = args.len();
                for a in args {
                    self.expr(&a.value)?;
                }
                self.emit(OpCode::Yield(n));
                Ok(())
            }

            Expr::Get { object, name } => {
                self.expr(object)?;
                let idx = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                self.emit(OpCode::GetField(idx));
                Ok(())
            }

            Expr::SafeGet { object, name } => {
                self.expr(object)?;
                let idx = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                self.emit(OpCode::GetFieldSafe(idx));
                Ok(())
            }

            Expr::Set { object, name, value } => {
                self.expr(object)?;
                self.expr(value)?;
                let idx = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                self.emit(OpCode::SetField(idx));
                Ok(())
            }

            Expr::ListLit(elems) => {
                let n = elems.len();
                for e in elems {
                    self.expr(e)?;
                }
                self.emit(OpCode::BuildList(n));
                Ok(())
            }

            Expr::MapLit(pairs) => {
                let n = pairs.len();
                for (key, val_expr) in pairs {
                    let idx = self.state_mut().chunk.add_constant(Constant::Str(key.clone()));
                    self.emit(OpCode::Constant(idx));
                    self.expr(val_expr)?;
                }
                self.emit(OpCode::BuildMap(n));
                Ok(())
            }

            Expr::Range { from, to } => {
                self.expr(from)?;
                self.expr(to)?;
                self.emit(OpCode::BuildRange);
                Ok(())
            }

            Expr::Index { object, index } => {
                self.expr(object)?;
                self.expr(index)?;
                self.emit(OpCode::Index);
                Ok(())
            }

            Expr::IndexSet { object, index, value } => {
                self.expr(object)?;
                self.expr(index)?;
                self.expr(value)?;
                self.emit(OpCode::IndexSet);
                Ok(())
            }

            Expr::StringInterp(parts) => {
                let n = parts.len();
                for part in parts {
                    match part {
                        StringPart::Lit(s) => {
                            let idx = self.state_mut().chunk.add_constant(Constant::Str(s.clone()));
                            self.emit(OpCode::Constant(idx));
                        }
                        StringPart::Expr(inner) => {
                            self.expr(inner)?;
                        }
                    }
                }
                self.emit(OpCode::BuildString(n));
                Ok(())
            }

            Expr::Super { method, args, block: _ } => {
                // Push self (slot 0 of the current method frame).
                self.emit(OpCode::GetSelf);
                let arg_count = args.len();
                for arg in args {
                    self.expr(&arg.value)?;
                }
                let name_idx = self.state_mut().chunk.add_constant(Constant::Str(method.clone()));
                self.emit(OpCode::SuperInvoke(name_idx, arg_count));
                Ok(())
            }

            #[allow(unreachable_patterns)]
            other => Err(self.error(format!(
                "expression not yet supported by compiler: {:?}",
                std::mem::discriminant(other)
            ))),
        }
    }

    fn literal(&mut self, val: &Value) -> Result<(), CompileError> {
        match val {
            Value::Bool(true)  => { self.emit(OpCode::True);  Ok(()) }
            Value::Bool(false) => { self.emit(OpCode::False); Ok(()) }
            Value::Nil         => { self.emit(OpCode::Nil);   Ok(()) }
            Value::Int(n) => {
                let idx = self.state_mut().chunk.add_constant(Constant::Int(*n));
                self.emit(OpCode::Constant(idx));
                Ok(())
            }
            Value::Float(n) => {
                let idx = self.state_mut().chunk.add_constant(Constant::Float(*n));
                self.emit(OpCode::Constant(idx));
                Ok(())
            }
            Value::Str(s) => {
                let idx = self.state_mut().chunk.add_constant(Constant::Str(s.clone()));
                self.emit(OpCode::Constant(idx));
                Ok(())
            }
            other => Err(self.error(format!(
                "literal not yet supported by compiler: {:?}",
                std::mem::discriminant(other)
            ))),
        }
    }

    // ── Variable resolution ───────────────────────────────────────────────────

    /// Look up `name` as a local in the function at `depth` (index into `states`).
    /// Returns the stack slot index if found.
    fn resolve_local(&self, depth: usize, name: &str) -> Option<usize> {
        self.states[depth].locals.iter().rposition(|l| l.name == name)
    }

    /// Resolve `name` as an upvalue visible from the function at `depth`.
    /// If found in a parent scope, the necessary `UpvalueDef` entries are added
    /// and the captured locals are marked.  Returns the upvalue index.
    fn resolve_upvalue(&mut self, depth: usize, name: &str) -> Option<usize> {
        if depth == 0 {
            return None; // top-level scope: no enclosing function
        }
        let parent = depth - 1;

        // Is it a local in the immediately enclosing function?
        if let Some(local_idx) = self.resolve_local(parent, name) {
            self.states[parent].locals[local_idx].captured = true;
            return Some(self.add_upvalue(depth, true, local_idx));
        }

        // Is it an upvalue in the enclosing function (transitive capture)?
        if let Some(upvalue_idx) = self.resolve_upvalue(parent, name) {
            return Some(self.add_upvalue(depth, false, upvalue_idx));
        }

        None
    }

    /// Add an upvalue descriptor to the function at `depth`, deduplicating.
    fn add_upvalue(&mut self, depth: usize, is_local: bool, index: usize) -> usize {
        let existing = self.states[depth].upvalue_defs.iter().position(|uv| {
            uv.is_local == is_local && uv.index == index
        });
        if let Some(i) = existing {
            return i;
        }
        let i = self.states[depth].upvalue_defs.len();
        self.states[depth].upvalue_defs.push(UpvalueInfo { is_local, index });
        i
    }

    /// True when `expr` is an assignment to a name that is neither an existing
    /// local nor an upvalue — i.e. it will create a new stack slot.
    fn is_new_local_assign(&self, expr: &Expr) -> bool {
        if let Expr::Assign { name, .. } = expr {
            let depth = self.states.len() - 1;
            self.resolve_local(depth, name).is_none()
                && !self.would_be_upvalue(depth, name)
        } else {
            false
        }
    }

    /// Pure (non-mutating) upvalue check used by `is_new_local_assign`.
    fn would_be_upvalue(&self, depth: usize, name: &str) -> bool {
        if depth == 0 { return false; }
        let parent = depth - 1;
        self.resolve_local(parent, name).is_some() || self.would_be_upvalue(parent, name)
    }

    // ── Emit helpers ──────────────────────────────────────────────────────────

    fn emit(&mut self, op: OpCode) {
        let line = self.state().current_line;
        self.state_mut().chunk.write(op, line);
    }

    fn emit_jump(&mut self, op: OpCode) -> usize {
        let idx = self.state().chunk.code.len();
        self.emit(op);
        idx
    }

    fn emit_loop(&mut self, loop_start: usize) {
        let offset = self.state().chunk.code.len() + 1 - loop_start;
        self.emit(OpCode::Loop(offset));
    }

    fn patch_jump(&mut self, jump_idx: usize) {
        self.state_mut().chunk.patch_jump(jump_idx);
    }

    /// Compile a `begin … rescue [e] … else … end` block.
    ///
    /// Generated bytecode layout:
    /// ```text
    /// BeginRescue { handler_offset: ?, rescue_var_slot }
    /// … body …
    /// PopRescue
    /// Jump(end_ip)          ← skip over rescue + else
    /// [handler_ip]:
    /// … rescue_body …
    /// Jump(else_ip)         ← skip else if rescue ran
    /// [else_ip]:
    /// … else_body …
    /// [end_ip]:
    /// ```
    fn compile_begin(
        &mut self,
        body:        &[Stmt],
        rescue_var:  &Option<String>,
        rescue_body: &[Stmt],
        else_body:   &[Stmt],
    ) -> Result<(), CompileError> {
        // Allocate a local slot for the rescue variable if one is named.
        let rescue_var_slot = if let Some(name) = rescue_var {
            let slot = self.state().locals.len();
            self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
            // Initialise to nil so the slot exists in the stack frame before BeginRescue.
            self.emit(OpCode::Nil);
            slot
        } else {
            usize::MAX
        };

        let begin_idx = self.emit_begin_rescue(rescue_var_slot);

        self.stmts(body)?;

        self.emit(OpCode::PopRescue);
        let jump_over_rescue = self.emit_jump(OpCode::Jump(0));

        // Patch handler_offset in BeginRescue to point here.
        self.patch_rescue(begin_idx);

        self.stmts(rescue_body)?;

        let jump_over_else = self.emit_jump(OpCode::Jump(0));

        self.patch_jump(jump_over_rescue);

        self.stmts(else_body)?;

        self.patch_jump(jump_over_else);

        Ok(())
    }

    fn emit_begin_rescue(&mut self, rescue_var_slot: usize) -> usize {
        let idx = self.state().chunk.code.len();
        self.emit(OpCode::BeginRescue { handler_offset: 0, rescue_var_slot });
        idx
    }

    fn patch_rescue(&mut self, begin_idx: usize) {
        self.state_mut().chunk.patch_rescue(begin_idx);
    }

    /// Compile a block literal into a closure and push it onto the stack.
    /// The block's params become local slots 1..n (slot 0 is the closure itself).
    fn compile_block(&mut self, block: &Block) -> Result<(), CompileError> {
        let line  = self.state().current_line;
        let arity = block.params.len();
        self.push_fn("<block>", arity, line);
        // Slot 0 = closure (anonymous; blocks don't recurse by name).
        self.state_mut().locals.push(LocalInfo { name: String::new(), captured: false });
        for p in &block.params {
            self.state_mut().locals.push(LocalInfo { name: p.clone(), captured: false });
        }
        self.compile_body(&block.body)?;
        let func = self.pop_fn();
        let idx  = self.state_mut().chunk.add_constant(Constant::Function(func));
        self.emit(OpCode::Closure(idx));
        Ok(())
    }

    fn stmts(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        for s in stmts { self.stmt(s)?; }
        Ok(())
    }

    /// Compile a class definition: emit each method as a closure, then emit
    /// `DefClass` which collects them into a Class value stored as a local.
    fn compile_class(
        &mut self,
        name:       &str,
        superclass: Option<&str>,
        fields:     &[crate::ast::FieldDef],
        methods:    &[MethodDef],
    ) -> Result<(), CompileError> {
        let line = self.state().current_line;

        for method in methods {
            let arity = method.params.len();
            self.push_fn(&method.name, arity, line);
            // Slot 0 = `self` (the receiver); it is NOT counted in `arity`.
            self.state_mut().locals.push(LocalInfo { name: "self".into(), captured: false });
            for p in &method.params {
                self.state_mut().locals.push(LocalInfo { name: p.name.clone(), captured: false });
            }
            self.compile_body(&method.body)?;
            let func = self.pop_fn();
            let fi = self.state_mut().chunk.add_constant(Constant::Function(func));
            self.emit(OpCode::Closure(fi));
        }

        let field_names:    Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        let field_defaults: Vec<Option<Constant>> = fields.iter().map(|f| {
            match &f.default {
                Some(Expr::Literal(Value::Int(n)))   => Some(Constant::Int(*n)),
                Some(Expr::Literal(Value::Float(n))) => Some(Constant::Float(*n)),
                Some(Expr::Literal(Value::Str(s)))   => Some(Constant::Str(s.clone())),
                _ => None,
            }
        }).collect();
        let method_names: Vec<String> = methods.iter().map(|m| m.name.clone()).collect();
        let desc_idx = self.state_mut().chunk.add_constant(Constant::ClassDesc {
            name:       name.to_string(),
            superclass: superclass.map(|s| s.to_string()),
            field_names,
            field_defaults,
            method_names,
        });
        self.emit(OpCode::DefClass(desc_idx));
        // Store the class value in a local slot named after the class.
        self.state_mut().locals.push(LocalInfo { name: name.to_string(), captured: false });
        Ok(())
    }

    fn error(&self, message: String) -> CompileError {
        CompileError { message, line: self.state().current_line }
    }
}

// Re-export compile as the public API.
pub fn compile(stmts: &[Stmt]) -> Result<Rc<Function>, CompileError> {
    Compiler::compile(stmts)
}
