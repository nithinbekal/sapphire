//! Direct bytecode / chunk tests (no lexer/parser/compiler).

use sapphire::chunk::{Chunk, Constant, Function, OpCode};
use sapphire::vm::{Vm, VmError, VmValue};
use std::path::PathBuf;
use std::rc::Rc;

fn run(chunk: Chunk) -> Result<Option<VmValue>, VmError> {
    let f = Rc::new(Function {
        name: String::new(),
        arity: 0,
        chunk,
        upvalue_defs: vec![],
        return_type: None,
    });
    Vm::new(f, PathBuf::new()).run()
}

fn chunk_with(ops: impl Fn(&mut Chunk)) -> Chunk {
    let mut c = Chunk::new();
    ops(&mut c);
    c
}

#[test]
fn bytecode_returns_constant() {
    let chunk = chunk_with(|c| {
        let i = c.add_constant(Constant::Int(42));
        c.write(OpCode::Constant(i), 1);
        c.write(OpCode::Return, 1);
    });
    assert_eq!(run(chunk).unwrap(), Some(VmValue::Int(42)));
}

#[test]
fn bytecode_addition_int() {
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
fn bytecode_addition_mixed() {
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
fn bytecode_negation() {
    let chunk = chunk_with(|c| {
        let i = c.add_constant(Constant::Int(7));
        c.write(OpCode::Constant(i), 1);
        c.write(OpCode::Negate, 1);
        c.write(OpCode::Return, 1);
    });
    assert_eq!(run(chunk).unwrap(), Some(VmValue::Int(-7)));
}

#[test]
fn bytecode_not_false_is_true() {
    let chunk = chunk_with(|c| {
        c.write(OpCode::False, 1);
        c.write(OpCode::Not, 1);
        c.write(OpCode::Return, 1);
    });
    assert_eq!(run(chunk).unwrap(), Some(VmValue::Bool(true)));
}

#[test]
fn bytecode_comparison_less() {
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
fn bytecode_string_concat() {
    let chunk = chunk_with(|c| {
        let a = c.add_constant(Constant::Str("hello".into()));
        let b = c.add_constant(Constant::Str(" world".into()));
        c.write(OpCode::Constant(a), 1);
        c.write(OpCode::Constant(b), 1);
        c.write(OpCode::Add, 1);
        c.write(OpCode::Return, 1);
    });
    assert_eq!(
        run(chunk).unwrap(),
        Some(VmValue::Str("hello world".into()))
    );
}

#[test]
fn bytecode_type_error_on_bad_add() {
    let chunk = chunk_with(|c| {
        let a = c.add_constant(Constant::Int(1));
        c.write(OpCode::Constant(a), 1);
        c.write(OpCode::True, 1);
        c.write(OpCode::Add, 1);
        c.write(OpCode::Return, 1);
    });
    assert!(matches!(run(chunk), Err(VmError::TypeError { .. })));
}
