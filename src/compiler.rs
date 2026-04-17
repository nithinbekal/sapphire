use std::fmt;
use std::rc::Rc;
use crate::ast::{Block, Expr, StringPart, MethodDef, TypeExpr};
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

/// Tracks the context needed to compile `break`/`next` inside a while loop.
struct LoopCtx {
    /// Instruction index of the loop condition check (target for `next`).
    start:       usize,
    /// Indices of `Jump(0)` placeholders emitted for `break` — patched after the loop.
    break_jumps: Vec<usize>,
}

struct FunctionState {
    chunk:        Chunk,
    current_line: u32,
    locals:       Vec<LocalInfo>,
    upvalue_defs: Vec<UpvalueInfo>,
    name:         String,
    arity:        usize,
    /// Stack of active while-loop contexts, innermost last.
    loop_stack:   Vec<LoopCtx>,
    return_type:  Option<String>,
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
            loop_stack:   Vec::new(),
            return_type:  None,
        }
    }
}

// ── Compiler ──────────────────────────────────────────────────────────────────

/// A stack of `FunctionState`s, one per nesting level.  The top of the stack
/// is the function currently being compiled.
struct Compiler {
    states:      Vec<FunctionState>,
    /// When true (REPL mode), top-level variable assignments and definitions
    /// are stored as globals rather than stack slots.
    global_mode: bool,
    /// True when the file contains at least one `import` statement.  Enables
    /// `GetGlobal` fallback for unresolved variable references so that names
    /// defined in imported files (which run as globals) are accessible.
    has_imports: bool,
}

impl Compiler {
    fn new() -> Self {
        Compiler { states: Vec::new(), global_mode: false, has_imports: false }
    }

    fn new_with_imports() -> Self {
        Compiler { states: Vec::new(), global_mode: false, has_imports: true }
    }

    fn new_repl() -> Self {
        Compiler { states: Vec::new(), global_mode: true, has_imports: false }
    }

    // ── Public entry points ───────────────────────────────────────────────────

    /// Compile a top-level program (arity 0, anonymous).
    pub fn compile(exprs: &[Expr]) -> Result<Rc<Function>, CompileError> {
        let has_imports = exprs.iter().any(|e| matches!(e, Expr::Import { .. }));
        let mut c = if has_imports { Compiler::new_with_imports() } else { Compiler::new() };
        c.push_fn("", 0, 0);
        c.compile_body(exprs)?;
        Ok(c.pop_fn())
    }

    /// Compile a REPL snippet: top-level variables go to globals instead of
    /// stack slots, so they persist across successive `eval()` calls.
    pub fn compile_repl(exprs: &[Expr]) -> Result<Rc<Function>, CompileError> {
        let mut c = Compiler::new_repl();
        c.push_fn("", 0, 0);
        c.compile_body(exprs)?;
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
            return_type: state.return_type,
        })
    }

    fn state(&self) -> &FunctionState {
        self.states.last().unwrap()
    }

    fn state_mut(&mut self) -> &mut FunctionState {
        self.states.last_mut().unwrap()
    }

    // ── Body compilation (shared between top-level and functions) ─────────────

    /// Compile a list of expressions, treating the last as an implicit return value (Ruby-style).
    fn compile_body(&mut self, exprs: &[Expr]) -> Result<(), CompileError> {
        let (last, rest) = match exprs.split_last() {
            Some(pair) => pair,
            None => {
                self.emit(OpCode::Nil);
                self.emit(OpCode::Return);
                return Ok(());
            }
        };
        self.stmts(rest)?;
        match last {
            Expr::Function { name, .. } => {
                self.expr(last)?;
                let idx = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                self.emit(OpCode::Constant(idx));
                self.emit(OpCode::Return);
            }
            e if !Self::is_stmt_form(e) => {
                self.expr(last)?;
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

    /// Branch body for `if` / `begin`: leave one value on the stack (no `Return`).
    fn compile_branch(&mut self, exprs: &[Expr]) -> Result<(), CompileError> {
        match exprs.split_last() {
            None => {
                self.emit(OpCode::Nil);
            }
            Some((last, rest)) => {
                self.stmts(rest)?;
                match last {
                    e if !Self::is_stmt_form(e) => {
                        self.expr(last)?;
                    }
                    other => {
                        self.stmt(other)?;
                        self.emit(OpCode::Nil);
                    }
                }
            }
        }
        Ok(())
    }

    fn is_stmt_form(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::While { .. }
                | Expr::Return(_)
                | Expr::Break(_)
                | Expr::Next(_)
                | Expr::Raise(_)
                | Expr::MultiAssign { .. }
                | Expr::Import { .. }
        )
    }

    // ── Statement-shaped expressions (also used from `expr()` for value position) ──

    fn stmt(&mut self, stmt: &Expr) -> Result<(), CompileError> {
        match stmt {
            Expr::Return(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Return);
            }

            Expr::While { condition, body } => {
                // Pre-declare any variables that are first introduced in the body so
                // that subsequent iterations reuse the same stack slot (SetLocal) rather
                // than pushing a new slot each time.
                self.hoist_while_locals(body);

                let loop_start = self.state().chunk.code.len();

                self.expr(condition.as_ref())?;
                let exit_jump = self.emit_jump(OpCode::JumpIfFalse(0));

                self.state_mut().loop_stack.push(LoopCtx { start: loop_start, break_jumps: vec![] });
                self.stmts(body)?;
                let ctx = self.state_mut().loop_stack.pop().unwrap();

                self.emit_loop(loop_start);
                self.patch_jump(exit_jump);

                for jump_idx in ctx.break_jumps {
                    self.patch_jump(jump_idx);
                }
            }

            Expr::Raise(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Raise);
            }

            Expr::Break(expr) => {
                self.expr(expr)?;
                if self.state().loop_stack.is_empty() {
                    self.emit(OpCode::Break);
                } else {
                    // Inside a while loop: discard the value, jump to after the loop.
                    self.emit(OpCode::Pop);
                    let jump_idx = self.emit_jump(OpCode::Jump(0));
                    self.state_mut().loop_stack.last_mut().unwrap().break_jumps.push(jump_idx);
                }
            }

            Expr::Next(expr) => {
                self.expr(expr)?;
                if self.state().loop_stack.is_empty() {
                    self.emit(OpCode::Next);
                } else {
                    // Inside a while loop: discard the value, jump back to condition.
                    self.emit(OpCode::Pop);
                    let start = self.state().loop_stack.last().unwrap().start;
                    self.emit_loop(start);
                }
            }

            Expr::MultiAssign { names, values } => {
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

            Expr::Import { path } => {
                let idx = self.state_mut().chunk.add_constant(Constant::Str(path.clone()));
                self.emit(OpCode::Import(idx));
            }

            e => {
                let is_new_local = self.is_new_local_assign(e);
                // In global mode, function/class defs emit SetGlobal (peek) instead of
                // leaving the value as a new local slot, so we must Pop afterwards.
                let defines_binding = !self.global_mode
                    && matches!(e, Expr::Function { .. } | Expr::Class { .. });
                self.expr(e)?;
                if !is_new_local && !defines_binding {
                    self.emit(OpCode::Pop);
                }
            }
        }
        Ok(())
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    fn expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Return(_)
            | Expr::Break(_)
            | Expr::Next(_)
            | Expr::Raise(_) => self.stmt(expr),

            Expr::While { .. } | Expr::MultiAssign { .. } | Expr::Import { .. } => {
                self.stmt(expr)?;
                self.emit(OpCode::Nil);
                Ok(())
            }

            Expr::Literal(val) => self.literal(val),

            Expr::Grouping(inner) => self.expr(inner),

            Expr::Unary { op, right } => {
                self.state_mut().current_line = op.line as u32;
                self.expr(right)?;
                match &op.kind {
                    TokenKind::Minus => self.emit(OpCode::Negate),
                    TokenKind::Bang  => self.emit(OpCode::Not),
                    TokenKind::Tilde => self.emit(OpCode::BitNot),
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
                    TokenKind::Plus           => self.emit(OpCode::Add),
                    TokenKind::Minus          => self.emit(OpCode::Sub),
                    TokenKind::Star           => self.emit(OpCode::Mul),
                    TokenKind::Slash          => self.emit(OpCode::Div),
                    TokenKind::Percent        => self.emit(OpCode::Mod),
                    TokenKind::Amp            => self.emit(OpCode::BitAnd),
                    TokenKind::Pipe           => self.emit(OpCode::BitOr),
                    TokenKind::Caret          => self.emit(OpCode::BitXor),
                    TokenKind::LessLess       => self.emit(OpCode::Shl),
                    TokenKind::GreaterGreater => self.emit(OpCode::Shr),
                    TokenKind::EqEq           => self.emit(OpCode::Equal),
                    TokenKind::BangEq         => self.emit(OpCode::NotEqual),
                    TokenKind::Less           => self.emit(OpCode::Less),
                    TokenKind::LessEq         => self.emit(OpCode::LessEqual),
                    TokenKind::Greater        => self.emit(OpCode::Greater),
                    TokenKind::GreaterEq      => self.emit(OpCode::GreaterEqual),
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
                } else if self.global_mode || self.has_imports
                    || name.starts_with(|c: char| c.is_uppercase())
                {
                    let idx = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                    self.emit(OpCode::GetGlobal(idx));
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
                } else if self.global_mode && depth == 0 {
                    // REPL top-level: persist as a global (peek semantics — TOS stays).
                    let idx = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                    self.emit(OpCode::SetGlobal(idx));
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

            Expr::Print(inner) => {
                self.expr(inner)?;
                self.emit(OpCode::Print);
                Ok(())
            }

            Expr::Class { name, superclass, fields, methods, nested, constants } => {
                self.compile_class(name, superclass.as_deref(), fields, methods, nested, constants, true)?;
                Ok(())
            }

            Expr::Function { name, params, return_type, body } => {
                let line  = self.state().current_line;
                let arity = params.len();

                self.push_fn(name, arity, line);
                self.state_mut().return_type = type_expr_name(return_type.as_ref());
                // Slot 0 = the function itself — enables direct recursion by name.
                self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
                for p in params {
                    self.state_mut().locals.push(LocalInfo { name: p.name.clone(), captured: false });
                }
                self.compile_body(body)?;
                let func = self.pop_fn();

                let idx = self.state_mut().chunk.add_constant(Constant::Function(func));
                self.emit(OpCode::Closure(idx));

                if self.global_mode && self.states.len() == 1 {
                    // REPL top-level: store in globals, don't allocate a stack slot.
                    let ni = self.state_mut().chunk.add_constant(Constant::Str(name.clone()));
                    self.emit(OpCode::SetGlobal(ni));
                } else {
                    // The closure value on the stack becomes this local's slot.
                    self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
                }
                Ok(())
            }

            Expr::If { condition, then_branch, else_branch } => {
                self.expr(condition)?;
                let jif = self.emit_jump(OpCode::JumpIfFalse(0));
                self.compile_branch(then_branch)?;
                match else_branch {
                    Some(else_stmts) => {
                        let jelse = self.emit_jump(OpCode::Jump(0));
                        self.patch_jump(jif);
                        self.compile_branch(else_stmts)?;
                        self.patch_jump(jelse);
                    }
                    None => {
                        let jend = self.emit_jump(OpCode::Jump(0));
                        self.patch_jump(jif);
                        self.emit(OpCode::Nil);
                        self.patch_jump(jend);
                    }
                }
                Ok(())
            }

            Expr::Begin {
                body,
                rescue_var,
                rescue_body,
                else_body,
            } => self.compile_begin_expr(body, rescue_var, rescue_body, else_body),

            Expr::Lambda { params, body } => {
                let line = self.state().current_line;
                let arity = params.len();
                self.push_fn("<lambda>", arity, line);
                // Slot 0 = the closure itself (mirrors named-function convention).
                self.state_mut().locals.push(LocalInfo { name: "<lambda>".to_string(), captured: false });
                for p in params {
                    self.state_mut().locals.push(LocalInfo { name: p.clone(), captured: false });
                }
                self.compile_body(body)?;
                let func = self.pop_fn();
                let idx = self.state_mut().chunk.add_constant(Constant::Function(func));
                self.emit(OpCode::Closure(idx));
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
        let depth = self.states.len() - 1;
        if self.global_mode && depth == 0 {
            // At the top level in global mode, assigns go to globals via SetGlobal
            // (peek semantics), so the value stays on the stack and stmt must Pop it.
            return false;
        }
        if let Expr::Assign { name, .. } = expr {
            self.resolve_local(depth, name).is_none()
                && !self.would_be_upvalue(depth, name)
        } else {
            false
        }
    }

    /// Pre-declare variables that are first introduced at the top level of a while
    /// body, emitting `Nil` for each so they occupy a fixed stack slot before the
    /// loop's condition is checked.  This prevents subsequent iterations from
    /// pushing a new stack slot instead of updating the existing one.
    fn hoist_while_locals(&mut self, body: &[Expr]) {
        if self.global_mode {
            return; // globals use SetGlobal, not stack slots
        }
        let depth = self.states.len() - 1;
        for stmt in body {
            match stmt {
                Expr::Assign { name, .. } => {
                    if self.resolve_local(depth, name).is_none()
                        && !self.would_be_upvalue(depth, name)
                    {
                        self.emit(OpCode::Nil);
                        self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
                    }
                }
                Expr::MultiAssign { names, .. } => {
                    for name in names {
                        if self.resolve_local(depth, name).is_none()
                            && !self.would_be_upvalue(depth, name)
                        {
                            self.emit(OpCode::Nil);
                            self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
                        }
                    }
                }
                _ => {}
            }
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

    /// Compile `begin … rescue … else … end` as an expression: one value on the stack.
    ///
    /// On success, if `else_body` is non-empty the Ruby value is the else branch (body’s
    /// value is discarded).
    fn compile_begin_expr(
        &mut self,
        body: &[Expr],
        rescue_var: &Option<String>,
        rescue_body: &[Expr],
        else_body: &[Expr],
    ) -> Result<(), CompileError> {
        let rescue_var_slot = if let Some(name) = rescue_var {
            let slot = self.state().locals.len();
            self.state_mut().locals.push(LocalInfo { name: name.clone(), captured: false });
            self.emit(OpCode::Nil);
            slot
        } else {
            usize::MAX
        };

        // Track locals before body so we can pad the rescue path later.
        let locals_before_body = self.state().locals.len();

        // Check BEFORE compiling body: is_new_local_assign checks whether the name is already
        // registered — after body compilation it would be, giving a false negative.
        let body_creates_new_local_result = body.last().is_some_and(|e| self.is_new_local_assign(e));

        let begin_idx = self.emit_begin_rescue(rescue_var_slot);

        self.compile_branch(body)?;

        let new_body_locals = self.state().locals.len() - locals_before_body;

        // If body ends with a new-local assign, TOS IS the local's stack slot (no SetLocal
        // was emitted — the push IS the slot). Push a copy so the begin expression's result
        // is a fresh value sitting above all the local slots.
        if body_creates_new_local_result {
            let slot = self.state().locals.len() - 1;
            self.emit(OpCode::GetLocal(slot));
        }

        self.emit(OpCode::PopRescue);
        let jump_over_rescue = self.emit_jump(OpCode::Jump(0));

        self.patch_rescue(begin_idx);

        // On the rescue path the stack is truncated back to pre-body height, losing any
        // slots that were allocated for body locals. Emit Nils to restore those slots so the
        // compiler's locals list stays in sync with the runtime stack layout.
        for _ in 0..new_body_locals {
            self.emit(OpCode::Nil);
        }

        self.compile_branch(rescue_body)?;

        let jump_over_else = self.emit_jump(OpCode::Jump(0));

        self.patch_jump(jump_over_rescue);

        if !else_body.is_empty() {
            self.emit(OpCode::Pop);
            self.compile_branch(else_body)?;
        }

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

    fn stmts(&mut self, exprs: &[Expr]) -> Result<(), CompileError> {
        for e in exprs { self.stmt(e)?; }
        Ok(())
    }

    /// Compile a class definition: emit each method as a closure, then emit
    /// `DefClass` which collects them into a Class value stored as a local.
    ///
    /// `bind` — when `true` (normal top-level class), store the result in a
    /// named local/global slot.  When `false` (nested class), leave the value
    /// on the stack for the enclosing `DefClass` to pick up.
    fn compile_class(
        &mut self,
        name:       &str,
        superclass: Option<&Expr>,
        fields:     &[crate::ast::FieldDef],
        methods:    &[MethodDef],
        nested:     &[Expr],
        constants:  &[(String, Box<Expr>)],
        bind:       bool,
    ) -> Result<(), CompileError> {
        let line = self.state().current_line;

        let (instance_methods, class_methods): (Vec<&MethodDef>, Vec<&MethodDef>) =
            methods.iter().partition(|m| !m.class_method);

        // Emit class method closures first (DefClass pops them in order).
        // Slot 0 = `self` (the class object), not counted in arity.
        for method in &class_methods {
            let arity = method.params.len();
            self.push_fn(&method.name, arity, line);
            self.state_mut().return_type = type_expr_name(method.return_type.as_ref());
            self.state_mut().locals.push(LocalInfo { name: "self".into(), captured: false });
            for p in &method.params {
                self.state_mut().locals.push(LocalInfo { name: p.name.clone(), captured: false });
            }
            self.compile_body(&method.body)?;
            let func = self.pop_fn();
            let fi = self.state_mut().chunk.add_constant(Constant::Function(func));
            self.emit(OpCode::Closure(fi));
        }

        // Emit instance method closures.
        for method in &instance_methods {
            let arity = method.params.len();
            self.push_fn(&method.name, arity, line);
            self.state_mut().return_type = type_expr_name(method.return_type.as_ref());
            self.state_mut().locals.push(LocalInfo { name: "self".into(), captured: false });
            for p in &method.params {
                self.state_mut().locals.push(LocalInfo { name: p.name.clone(), captured: false });
            }
            self.compile_body(&method.body)?;
            let func = self.pop_fn();
            let fi = self.state_mut().chunk.add_constant(Constant::Function(func));
            self.emit(OpCode::Closure(fi));
        }

        // Emit nested class values (each leaves a Class value on the stack).
        let mut nested_class_names: Vec<String> = Vec::new();
        for nested_expr in nested {
            match nested_expr {
                Expr::Class { name: nname, superclass: nsuper, fields: nfields, methods: nmethods, nested: nnested, constants: nconsts } => {
                    nested_class_names.push(nname.clone());
                    self.compile_class(nname, nsuper.as_deref(), nfields, nmethods, nnested, nconsts, false)?;
                }
                _ => return Err(self.error("nested class body must be a class expression".to_string())),
            }
        }

        // Emit class constants (each leaves a value on the stack, stored in namespace).
        for (cname, cexpr) in constants {
            nested_class_names.push(cname.clone());
            self.expr(cexpr)?;
        }

        // Resolve superclass: static (simple Variable) vs dynamic (any other expr).
        let (static_super, superclass_dynamic) = match superclass {
            None => (None, false),
            Some(Expr::Variable(sname)) => (Some(sname.as_str()), false),
            Some(expr) => {
                // Dynamic: emit the expression — its value will be on TOS after
                // the nested class values, and DefClass will pop it.
                self.expr(expr)?;
                (None, true)
            }
        };

        let field_names:        Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        let field_defaults: Vec<Option<Constant>> = fields.iter().map(|f| {
            match &f.default {
                Some(Expr::Literal(Value::Int(n)))   => Some(Constant::Int(*n)),
                Some(Expr::Literal(Value::Float(n))) => Some(Constant::Float(*n)),
                Some(Expr::Literal(Value::Str(s)))   => Some(Constant::Str(s.clone())),
                _ => None,
            }
        }).collect();
        let method_names:       Vec<String> = instance_methods.iter().map(|m| m.name.clone()).collect();
        let private_methods:    Vec<String> = instance_methods.iter().filter(|m| m.private).map(|m| m.name.clone()).collect();
        let class_method_names: Vec<String> = class_methods.iter().map(|m| m.name.clone()).collect();
        let desc_idx = self.state_mut().chunk.add_constant(Constant::ClassDesc {
            name:               name.to_string(),
            superclass:         static_super.map(|s| s.to_string()),
            superclass_dynamic,
            field_names,
            field_defaults,
            method_names,
            private_methods,
            class_method_names,
            nested_class_names,
        });
        self.emit(OpCode::DefClass(desc_idx));

        if !bind {
            // Nested class: leave the Class value on the stack; the enclosing
            // DefClass will drain it.
            return Ok(());
        }

        if self.global_mode && self.states.len() == 1 {
            // REPL top-level: store in globals, don't allocate a stack slot.
            let ni = self.state_mut().chunk.add_constant(Constant::Str(name.to_string()));
            self.emit(OpCode::SetGlobal(ni));
        } else {
            // Store the class value in a local slot named after the class.
            self.state_mut().locals.push(LocalInfo { name: name.to_string(), captured: false });
        }
        Ok(())
    }

    fn error(&self, message: String) -> CompileError {
        CompileError { message, line: self.state().current_line }
    }
}

// Re-export compile as the public API.
pub fn compile(exprs: &[Expr]) -> Result<Rc<Function>, CompileError> {
    Compiler::compile(exprs)
}

pub fn compile_repl(exprs: &[Expr]) -> Result<Rc<Function>, CompileError> {
    Compiler::compile_repl(exprs)
}

/// Extract a plain type name string from an optional TypeExpr.
/// Returns `None` for absent annotations and `TypeExpr::Any`.
fn type_expr_name(te: Option<&TypeExpr>) -> Option<String> {
    match te {
        Some(TypeExpr::Named(n)) => Some(n.clone()),
        _ => None,
    }
}
