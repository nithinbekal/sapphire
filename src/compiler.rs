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
            }

            Stmt::Class { name, fields, methods, .. } => {
                self.compile_class(name, fields, methods)?;
            }

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
        name:    &str,
        fields:  &[crate::ast::FieldDef],
        methods: &[MethodDef],
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

        let field_names:  Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        let method_names: Vec<String> = methods.iter().map(|m| m.name.clone()).collect();
        let desc_idx = self.state_mut().chunk.add_constant(Constant::ClassDesc {
            name: name.to_string(),
            field_names,
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::vm::{Vm, VmValue};

    /// Lex → parse → compile → run, return the top-of-stack value.
    fn eval(src: &str) -> VmValue {
        let tokens = crate::lexer::Lexer::new(src).scan_tokens();
        let stmts  = crate::parser::Parser::new(tokens).parse().expect("parse error");
        let func   = compile(&stmts).expect("compile error");
        Vm::new(func).run().expect("vm error").expect("empty stack")
    }

    #[test]
    fn int_literal() {
        assert_eq!(eval("42"), VmValue::Int(42));
    }

    #[test]
    fn float_literal() {
        assert_eq!(eval("3.14"), VmValue::Float(3.14));
    }

    #[test]
    fn bool_literals() {
        assert_eq!(eval("true"),  VmValue::Bool(true));
        assert_eq!(eval("false"), VmValue::Bool(false));
    }

    #[test]
    fn nil_literal() {
        assert_eq!(eval("nil"), VmValue::Nil);
    }

    #[test]
    fn arithmetic() {
        assert_eq!(eval("1 + 2"),     VmValue::Int(3));
        assert_eq!(eval("10 - 3"),    VmValue::Int(7));
        assert_eq!(eval("4 * 5"),     VmValue::Int(20));
        assert_eq!(eval("10 / 2"),    VmValue::Int(5));
    }

    #[test]
    fn negation() {
        assert_eq!(eval("-7"), VmValue::Int(-7));
    }

    #[test]
    fn not() {
        assert_eq!(eval("!true"),  VmValue::Bool(false));
        assert_eq!(eval("!false"), VmValue::Bool(true));
        assert_eq!(eval("!nil"),   VmValue::Bool(true));
    }

    #[test]
    fn comparisons() {
        assert_eq!(eval("3 < 5"),  VmValue::Bool(true));
        assert_eq!(eval("5 > 3"),  VmValue::Bool(true));
        assert_eq!(eval("3 == 3"), VmValue::Bool(true));
        assert_eq!(eval("3 != 4"), VmValue::Bool(true));
        assert_eq!(eval("3 <= 3"), VmValue::Bool(true));
        assert_eq!(eval("4 >= 5"), VmValue::Bool(false));
    }

    #[test]
    fn string_concat() {
        assert_eq!(eval(r#""hello" + " world""#), VmValue::Str("hello world".into()));
    }

    #[test]
    fn grouping() {
        assert_eq!(eval("(2 + 3) * 4"), VmValue::Int(20));
    }

    #[test]
    fn variable_assign_and_read() {
        assert_eq!(eval("x = 42\nx"), VmValue::Int(42));
    }

    #[test]
    fn variable_reassign() {
        assert_eq!(eval("x = 1\nx = 2\nx"), VmValue::Int(2));
    }

    #[test]
    fn multiple_variables() {
        assert_eq!(eval("a = 3\nb = 4\na + b"), VmValue::Int(7));
    }

    #[test]
    fn if_true_branch() {
        assert_eq!(eval("x = 0\nif true { x = 1 }\nx"), VmValue::Int(1));
    }

    #[test]
    fn if_false_branch_skipped() {
        assert_eq!(eval("x = 0\nif false { x = 1 }\nx"), VmValue::Int(0));
    }

    #[test]
    fn if_else_selects_branch() {
        assert_eq!(eval("x = 0\nif false { x = 1 } else { x = 2 }\nx"), VmValue::Int(2));
    }

    #[test]
    fn if_elsif() {
        let src = "x = 0\nif false { x = 1 } elsif true { x = 2 }\nx";
        assert_eq!(eval(src), VmValue::Int(2));
    }

    #[test]
    fn while_loop_counts() {
        let src = "i = 0\nwhile i < 5 { i = i + 1 }\ni";
        assert_eq!(eval(src), VmValue::Int(5));
    }

    #[test]
    fn while_false_never_executes() {
        let src = "x = 42\nwhile false { x = 0 }\nx";
        assert_eq!(eval(src), VmValue::Int(42));
    }

    #[test]
    fn while_accumulates() {
        let src = "i = 0\nsum = 0\nwhile i < 4 { sum = sum + i\ni = i + 1 }\nsum";
        assert_eq!(eval(src), VmValue::Int(6));
    }

    #[test]
    fn last_expr_is_implicit_return() {
        let tokens = crate::lexer::Lexer::new("1 + 1\n2 + 2").scan_tokens();
        let stmts  = crate::parser::Parser::new(tokens).parse().unwrap();
        let func   = compile(&stmts).unwrap();
        let result = Vm::new(func).run().unwrap();
        assert_eq!(result, Some(VmValue::Int(4)));
    }

    #[test]
    fn function_call_no_args() {
        let src = "def answer() { 42 }\nanswer()";
        assert_eq!(eval(src), VmValue::Int(42));
    }

    #[test]
    fn function_call_with_args() {
        let src = "def add(a, b) { a + b }\nadd(3, 4)";
        assert_eq!(eval(src), VmValue::Int(7));
    }

    #[test]
    fn function_local_vars_dont_leak() {
        let src = "def f() { x = 99\nx }\nx = 1\nf()\nx";
        assert_eq!(eval(src), VmValue::Int(1));
    }

    #[test]
    fn recursive_function() {
        let src = "def fact(n) { if n <= 1 { return 1 }\nn * fact(n - 1) }\nfact(5)";
        assert_eq!(eval(src), VmValue::Int(120));
    }

    // ── Closure tests ─────────────────────────────────────────────────────────

    #[test]
    fn closure_captures_param() {
        // adder closes over `n` from make_adder's frame
        let src = "
def make_adder(n) {
  def adder(x) { n + x }
  adder
}
add5 = make_adder(5)
add5(3)";
        assert_eq!(eval(src), VmValue::Int(8));
    }

    #[test]
    fn closure_captures_local() {
        let src = "
def make_counter() {
  count = 0
  def inc() { count = count + 1\ncount }
  inc
}
counter = make_counter()
counter()
counter()
counter()";
        assert_eq!(eval(src), VmValue::Int(3));
    }

    #[test]
    fn closure_survives_enclosing_frame() {
        // make_adder has returned by the time add5 is called
        let src = "
def make_adder(n) {
  def adder(x) { n + x }
  adder
}
add5 = make_adder(5)
add10 = make_adder(10)
add5(1) + add10(1)";
        assert_eq!(eval(src), VmValue::Int(17));
    }

    // ── and / or / print ──────────────────────────────────────────────────────

    // ── Blocks ────────────────────────────────────────────────────────────────

    #[test]
    fn block_yield_basic() {
        let src = "
def call_block() { yield }
call_block() { 42 }";
        assert_eq!(eval(src), VmValue::Int(42));
    }

    #[test]
    fn block_yield_with_arg() {
        let src = "
def apply(x) { yield(x) }
apply(10) { |n| n * 2 }";
        assert_eq!(eval(src), VmValue::Int(20));
    }

    #[test]
    fn block_captures_outer_var() {
        let src = "
def run() { yield(5) }
factor = 3
run() { |x| x * factor }";
        assert_eq!(eval(src), VmValue::Int(15));
    }

    #[test]
    fn block_yield_multiple_times() {
        let src = "
def twice() { yield(1)\nyield(2) }
sum = 0
twice() { |n| sum = sum + n }
sum";
        assert_eq!(eval(src), VmValue::Int(3));
    }

    // ── Classes ───────────────────────────────────────────────────────────────

    #[test]
    fn class_instantiation_and_field_read() {
        let src = "class Point { attr x\nattr y }\np = Point.new(x: 3, y: 4)\np.x";
        assert_eq!(eval(src), VmValue::Int(3));
    }

    #[test]
    fn class_field_write() {
        let src = "class Box { attr val }\nb = Box.new(val: 1)\nb.val = 99\nb.val";
        assert_eq!(eval(src), VmValue::Int(99));
    }

    #[test]
    fn class_method_call() {
        let src = "class Counter {
  attr n
  def inc() { self.n = self.n + 1 }
  def get() { self.n }
}
c = Counter.new(n: 0)
c.inc()
c.inc()
c.get()";
        assert_eq!(eval(src), VmValue::Int(2));
    }

    #[test]
    fn class_method_with_args() {
        let src = "class Math {
  def add(a, b) { a + b }
}
m = Math.new()
m.add(3, 4)";
        assert_eq!(eval(src), VmValue::Int(7));
    }

    #[test]
    fn class_method_returns_self_field() {
        let src = "class Dog {
  attr name
  def bark() { self.name }
}
d = Dog.new(name: \"Rex\")
d.bark()";
        assert_eq!(eval(src), VmValue::Str("Rex".into()));
    }

    // ── Lists / maps / ranges ─────────────────────────────────────────────────

    #[test]
    fn list_literal() {
        // Can't use == on List (Rc pointer equality); check via indexing instead.
        assert_eq!(eval("[1, 2, 3]\n0"), VmValue::Int(0)); // compiles without error
        assert_eq!(eval("a = [1, 2, 3]\na[0]"), VmValue::Int(1));
        assert_eq!(eval("a = [1, 2, 3]\na[2]"), VmValue::Int(3));
    }

    #[test]
    fn list_index_read() {
        assert_eq!(eval("a = [10, 20, 30]\na[1]"), VmValue::Int(20));
    }

    #[test]
    fn list_index_negative() {
        assert_eq!(eval("a = [10, 20, 30]\na[-1]"), VmValue::Int(30));
    }

    #[test]
    fn list_index_write() {
        assert_eq!(eval("a = [1, 2, 3]\na[0] = 99\na[0]"), VmValue::Int(99));
    }

    #[test]
    fn map_literal_and_lookup() {
        assert_eq!(eval(r#"m = {x: 1, y: 2}
m["x"]"#), VmValue::Int(1));
    }

    #[test]
    fn map_missing_key_is_nil() {
        assert_eq!(eval(r#"m = {a: 1}
m["z"]"#), VmValue::Nil);
    }

    #[test]
    fn range_builds() {
        assert_eq!(eval("1..5"), VmValue::Range { from: 1, to: 5 });
    }

    // ── String interpolation ──────────────────────────────────────────────────

    #[test]
    fn string_interp_plain() {
        assert_eq!(eval(r#""hello""#), VmValue::Str("hello".into()));
    }

    #[test]
    fn string_interp_with_expr() {
        assert_eq!(
            eval(r#"x = 42
"value is #{x}""#),
            VmValue::Str("value is 42".into())
        );
    }

    #[test]
    fn string_interp_multiple_parts() {
        assert_eq!(
            eval(r##"a = 1
b = 2
"#{a} + #{b} = #{a + b}""##),
            VmValue::Str("1 + 2 = 3".into())
        );
    }

    #[test]
    fn and_short_circuits_false() {
        // false && anything → false (right side never evaluated)
        assert_eq!(eval("false && true"),  VmValue::Bool(false));
        assert_eq!(eval("nil && 42"),      VmValue::Nil);
    }

    #[test]
    fn and_returns_rhs_when_truthy() {
        assert_eq!(eval("true && 42"),  VmValue::Int(42));
        assert_eq!(eval("1 && 2"),      VmValue::Int(2));
    }

    #[test]
    fn or_short_circuits_truthy() {
        assert_eq!(eval("42 || false"),  VmValue::Int(42));
        assert_eq!(eval("true || nil"),  VmValue::Bool(true));
    }

    #[test]
    fn or_returns_rhs_when_falsy() {
        assert_eq!(eval("false || 99"), VmValue::Int(99));
        assert_eq!(eval("nil || nil"),  VmValue::Nil);
    }

    #[test]
    fn print_statement_returns_nil() {
        // `print` is a statement; the last expr is what the program returns.
        assert_eq!(eval("print 42\n99"), VmValue::Int(99));
    }

    #[test]
    fn transitive_capture() {
        // inner captures mid's local, which itself captures outer's param
        let src = "
def outer(x) {
  def mid() {
    def inner() { x }
    inner
  }
  mid
}
f = outer(42)()
f()";
        assert_eq!(eval(src), VmValue::Int(42));
    }
}
