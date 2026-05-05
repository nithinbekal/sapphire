use sapphire::compiler::compile;
use sapphire::lexer::Lexer;
use sapphire::parser::Parser;
use sapphire::vm::{Vm, VmError, VmValue};
use std::path::PathBuf;

fn eval(src: &str) -> VmValue {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    Vm::new(func, PathBuf::new())
        .run()
        .expect("vm error")
        .expect("empty stack")
}

fn eval_err(src: &str) -> VmError {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    Vm::new(func, PathBuf::new())
        .run()
        .expect_err("expected vm error")
}

fn typecheck_err_msg(src: &str) -> String {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens)
        .parse()
        .expect("parse error");
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(!errors.is_empty(), "expected type errors for:\n{src}");
    errors[0].message.clone()
}

macro_rules! assert_typecheck_error {
    ($src:expr, $($substring:expr),+ $(,)?) => {{
        let msg = typecheck_err_msg($src);
        $(
        assert!(
            msg.contains($substring),
            "expected first type error to contain:\n{}\n\nmessage:\n{}",
            $substring,
            msg
        );
        )*
    }};
}

#[test]
fn abstract_class_new_is_error() {
    let err = eval_err(
        "abstract class Shape {\n  abstract def area -> Float\n}\nShape.new()",
    );
    match err {
        VmError::TypeError { message, .. } => {
            assert!(message.contains("cannot instantiate abstract class Shape"));
        }
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn concrete_subclass_missing_abstract_method_errors_at_new() {
    let err = eval_err(
        "abstract class Shape {\n  abstract def area -> Float\n  abstract def perimeter -> Float\n}\nclass Broken < Shape {\n  def area -> Float { 0.0 }\n}\nBroken.new()",
    );
    match err {
        VmError::TypeError { message, .. } => {
            assert!(message.contains("Broken"));
            assert!(message.contains("perimeter"));
        }
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn concrete_subclass_implements_abstract_methods() {
    let src = "abstract class Shape {\n  abstract def area -> Float\n}\nclass Square < Shape {\n  def area -> Float { 4.0 }\n}\nSquare.new().area()";
    assert_eq!(eval(src), VmValue::Float(4.0));
}

#[test]
fn abstract_partial_parent_concrete_grandchild() {
    let src = "abstract class A {\n  abstract def foo -> Int\n  abstract def bar -> Int\n}\nabstract class B < A {\n  def foo -> Int { 1 }\n}\nclass C < B {\n  def bar -> Int { 2 }\n}\nC.new().foo() + C.new().bar()";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn concrete_method_on_abstract_class_callable_on_subclass() {
    let src = "abstract class Base {\n  abstract def m -> Int\n  def helper -> Int { 10 }\n}\nclass Child < Base {\n  def m -> Int { self.helper() + 1 }\n}\nChild.new().m()";
    assert_eq!(eval(src), VmValue::Int(11));
}

#[test]
fn abstract_def_in_non_abstract_class_errors() {
    assert_typecheck_error!(
        "class C {\n  abstract def m -> Int\n}",
        "abstract method 'm' is only allowed in an abstract class",
    );
}

#[test]
fn concrete_class_must_implement_inherited_abstract() {
    assert_typecheck_error!(
        "abstract class A {\n  abstract def m -> Int\n}\nclass B < A {\n}",
        "must implement abstract method: m",
    );
}
