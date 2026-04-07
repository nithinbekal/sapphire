use std::fmt;
use std::rc::Rc;
use crate::ast::{Expr, ParamDef, Stmt};
use crate::chunk::{Chunk, Constant, Function, OpCode};
use crate::token::TokenKind;
use crate::value::Value;

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub struct CompileError {
    pub message: String,
    pub line: u32,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[line {}] compile error: {}", self.line, self.message)
    }
}

// ── Compiler ─────────────────────────────────────────────────────────────────

struct Compiler {
    chunk:        Chunk,
    current_line: u32,
    /// Names of locals in declaration order; index = stack slot within this frame.
    locals:       Vec<String>,
}

impl Compiler {
    fn new(line: u32) -> Self {
        Compiler { chunk: Chunk::new(), current_line: line, locals: Vec::new() }
    }

    // ── Public entry points ───────────────────────────────────────────────

    /// Compile a top-level program (arity 0, anonymous).
    pub fn compile(stmts: &[Stmt]) -> Result<Rc<Function>, CompileError> {
        let mut c = Compiler::new(0);
        c.compile_body(stmts)?;
        Ok(Rc::new(Function { name: String::new(), arity: 0, chunk: c.chunk }))
    }

    /// Compile a named function with parameters.
    fn compile_fn(
        name: &str,
        params: &[ParamDef],
        body: &[Stmt],
        line: u32,
    ) -> Result<Rc<Function>, CompileError> {
        let mut c = Compiler::new(line);
        // Slot 0 is the function itself — enables direct recursion by name.
        c.locals.push(name.to_string());
        for p in params {
            c.locals.push(p.name.clone());
        }
        c.compile_body(body)?;
        Ok(Rc::new(Function { name: name.to_string(), arity: params.len(), chunk: c.chunk }))
    }

    // ── Body compilation (shared between top-level and functions) ─────────

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

    // ── Statements ────────────────────────────────────────────────────────

    fn stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Expression(expr) => {
                let is_new_local = self.defines_new_local(expr);
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
                let func = Compiler::compile_fn(name, params, body, self.current_line)?;
                let idx  = self.chunk.add_constant(Constant::Function(func));
                self.emit(OpCode::Constant(idx));
                // The function value on the stack becomes this local's slot.
                self.locals.push(name.clone());
            }

            Stmt::If { condition, then_branch, else_branch } => {
                self.expr(condition)?;
                let jif = self.emit_jump(OpCode::JumpIfFalse(0));

                self.stmts(then_branch)?;

                match else_branch {
                    Some(else_stmts) => {
                        let jump = self.emit_jump(OpCode::Jump(0));
                        self.chunk.patch_jump(jif);
                        self.stmts(else_stmts)?;
                        self.chunk.patch_jump(jump);
                    }
                    None => {
                        self.chunk.patch_jump(jif);
                    }
                }
            }

            Stmt::While { condition, body } => {
                let loop_start = self.chunk.code.len();

                self.expr(condition)?;
                let exit_jump = self.emit_jump(OpCode::JumpIfFalse(0));

                self.stmts(body)?;
                self.emit_loop(loop_start);

                self.chunk.patch_jump(exit_jump);
            }

            Stmt::Print(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Pop);
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

    // ── Expressions ───────────────────────────────────────────────────────

    fn expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Literal(val) => self.literal(val),

            Expr::Grouping(inner) => self.expr(inner),

            Expr::Unary { op, right } => {
                self.current_line = op.line as u32;
                self.expr(right)?;
                match &op.kind {
                    TokenKind::Minus => self.emit(OpCode::Negate),
                    TokenKind::Bang  => self.emit(OpCode::Not),
                    other => return Err(self.error(format!("unknown unary op: {:?}", other))),
                }
                Ok(())
            }

            Expr::Binary { left, op, right } => {
                self.current_line = op.line as u32;
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
                match self.resolve_local(name) {
                    Some(slot) => { self.emit(OpCode::GetLocal(slot)); Ok(()) }
                    None => Err(self.error(format!("undefined variable '{}'", name))),
                }
            }

            Expr::Assign { name, value } => {
                self.expr(value)?;
                match self.resolve_local(name) {
                    Some(slot) => { self.emit(OpCode::SetLocal(slot)); }
                    None       => { self.locals.push(name.clone()); }
                }
                Ok(())
            }

            Expr::Call { callee, args, block } => {
                if block.is_some() {
                    return Err(self.error("blocks not yet supported by compiler".into()));
                }
                self.expr(callee)?;
                let arg_count = args.len();
                for arg in args {
                    self.expr(&arg.value)?;
                }
                self.emit(OpCode::Call(arg_count));
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
                let idx = self.chunk.add_constant(Constant::Int(*n));
                self.emit(OpCode::Constant(idx));
                Ok(())
            }
            Value::Float(n) => {
                let idx = self.chunk.add_constant(Constant::Float(*n));
                self.emit(OpCode::Constant(idx));
                Ok(())
            }
            Value::Str(s) => {
                let idx = self.chunk.add_constant(Constant::Str(s.clone()));
                self.emit(OpCode::Constant(idx));
                Ok(())
            }
            other => Err(self.error(format!(
                "literal not yet supported by compiler: {:?}",
                std::mem::discriminant(other)
            ))),
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn emit(&mut self, op: OpCode) {
        self.chunk.write(op, self.current_line);
    }

    fn emit_jump(&mut self, op: OpCode) -> usize {
        let idx = self.chunk.code.len();
        self.emit(op);
        idx
    }

    fn emit_loop(&mut self, loop_start: usize) {
        let offset = self.chunk.code.len() + 1 - loop_start;
        self.emit(OpCode::Loop(offset));
    }

    fn stmts(&mut self, stmts: &[Stmt]) -> Result<(), CompileError> {
        for s in stmts { self.stmt(s)?; }
        Ok(())
    }

    fn resolve_local(&self, name: &str) -> Option<usize> {
        self.locals.iter().rposition(|n| n == name)
    }

    fn defines_new_local(&self, expr: &Expr) -> bool {
        matches!(expr, Expr::Assign { name, .. } if self.resolve_local(name).is_none())
    }

    fn error(&self, message: String) -> CompileError {
        CompileError { message, line: self.current_line }
    }
}

// Re-export compile as the public API.
pub fn compile(stmts: &[Stmt]) -> Result<Rc<Function>, CompileError> {
    Compiler::compile(stmts)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
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
}
