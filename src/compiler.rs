use std::fmt;
use crate::ast::{Expr, Stmt};
use crate::chunk::{Chunk, Constant, OpCode};
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

pub struct Compiler {
    chunk:        Chunk,
    current_line: u32,
}

impl Compiler {
    /// Compile a list of statements into a `Chunk`.
    ///
    /// If the last statement is a bare expression, its value is returned
    /// (implicit return, Ruby-style). Otherwise an implicit `nil` is returned.
    pub fn compile(stmts: &[Stmt]) -> Result<Chunk, CompileError> {
        let mut c = Compiler { chunk: Chunk::new(), current_line: 0 };
        let (last, rest) = match stmts.split_last() {
            Some(pair) => pair,
            None => {
                c.emit(OpCode::Nil);
                c.emit(OpCode::Return);
                return Ok(c.chunk);
            }
        };
        for stmt in rest {
            c.stmt(stmt)?;
        }
        // Last statement: if it's an expression, leave its value on the stack.
        match last {
            Stmt::Expression(expr) => {
                c.expr(expr)?;
                c.emit(OpCode::Return);
            }
            other => {
                c.stmt(other)?;
                c.emit(OpCode::Nil);
                c.emit(OpCode::Return);
            }
        }
        Ok(c.chunk)
    }

    // ── Statements ────────────────────────────────────────────────────────

    fn stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Expression(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Pop);
            }
            Stmt::Return(expr) => {
                self.expr(expr)?;
                self.emit(OpCode::Return);
            }
            Stmt::Print(expr) => {
                // The tree-walk interpreter has a built-in `print` statement.
                // For now we can't call native functions, so we compile the
                // expression and leave the value on the stack, then pop it.
                // A dedicated Print opcode can be added later.
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

    fn error(&self, message: String) -> CompileError {
        CompileError { message, line: self.current_line }
    }
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
        let chunk  = Compiler::compile(&stmts).expect("compile error");
        Vm::new(&chunk).run().expect("vm error").expect("empty stack")
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
    fn last_expr_is_implicit_return() {
        // All but the last expression statement are popped;
        // the last one is returned implicitly.
        let tokens = crate::lexer::Lexer::new("1 + 1\n2 + 2").scan_tokens();
        let stmts  = crate::parser::Parser::new(tokens).parse().unwrap();
        let chunk  = Compiler::compile(&stmts).unwrap();
        let result = Vm::new(&chunk).run().unwrap();
        assert_eq!(result, Some(VmValue::Int(4)));
    }
}
