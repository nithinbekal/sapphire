use sapphire::compiler::compile;
use sapphire::lexer::Lexer;
use sapphire::parser::Parser;
use sapphire::typechecker::TypeChecker;
use sapphire::vm::{Vm, VmValue};
use std::path::PathBuf;

fn parse(src: &str) -> Vec<sapphire::ast::Expr> {
    let tokens = Lexer::new(src).scan_tokens();
    Parser::new(tokens).parse().expect("parse error")
}

fn typecheck_ok(src: &str) {
    let stmts = parse(src);
    let errors = TypeChecker::check(&stmts);
    assert!(errors.is_empty(), "unexpected type errors: {:?}", errors);
}

fn typecheck_err_msg(src: &str) -> String {
    let stmts = parse(src);
    let errors = TypeChecker::check(&stmts);
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
        )+
    }};
}

fn eval(src: &str) -> VmValue {
    let stmts = parse(src);
    let func = compile(&stmts).expect("compile error");
    Vm::new(func, PathBuf::new())
        .run()
        .expect("vm error")
        .expect("empty stack")
}

#[test]
fn class_structurally_satisfies_interface_without_declaration() {
    typecheck_ok(
        r#"
interface Drawable {
  def draw -> String
}

class Circle {
  def draw -> String { "circle" }
}

def render(item: Drawable) -> String {
  item.draw()
}

render(Circle.new())
"#,
    );
}

#[test]
fn interface_annotation_is_static_only_at_runtime() {
    let value = eval(
        r#"
interface Drawable {
  def draw -> String
}

class Circle {
  def draw -> String { "circle" }
}

def render(item: Drawable) -> String {
  item.draw()
}

render(Circle.new())
"#,
    );

    assert_eq!(value, VmValue::Str("circle".into()));
}

#[test]
fn missing_interface_method_is_type_error() {
    assert_typecheck_error!(
        r#"
interface Drawable {
  def draw -> String
}

class Circle {
}

def render(item: Drawable) -> String {
  item.draw()
}

render(Circle.new())
"#,
        "Circle",
        "Drawable",
        "draw",
    );
}

#[test]
fn interface_method_return_must_match() {
    assert_typecheck_error!(
        r#"
interface Drawable {
  def draw -> String
}

class Circle {
  def draw -> Int { 1 }
}

def render(item: Drawable) -> String {
  item.draw()
}

render(Circle.new())
"#,
        "Circle",
        "Drawable",
        "draw",
        "expected String",
        "got Int",
    );
}

#[test]
fn interface_typed_value_only_exposes_interface_methods() {
    assert_typecheck_error!(
        r#"
interface Drawable {
  def draw -> String
}

class Circle {
  def draw -> String { "circle" }
  def radius -> Int { 5 }
}

def render(item: Drawable) -> Int {
  item.radius()
}

render(Circle.new())
"#,
        "method 'radius' is not defined by interface Drawable",
    );
}

#[test]
fn generic_interface_substitutes_type_arguments() {
    typecheck_ok(
        r#"
interface Sink[T] {
  def push(value: T) -> Nil
}

class StringSink {
  def push(value: String) -> Nil { nil }
}

def write(sink: Sink[String]) -> Nil {
  sink.push("hello")
}

write(StringSink.new())
"#,
    );
}

#[test]
fn generic_interface_rejects_wrong_type_argument() {
    assert_typecheck_error!(
        r#"
interface Sink[T] {
  def push(value: T) -> Nil
}

class IntSink {
  def push(value: Int) -> Nil { nil }
}

def write(sink: Sink[String]) -> Nil {
  sink.push("hello")
}

write(IntSink.new())
"#,
        "IntSink",
        "Sink[String]",
        "push",
        "expected String",
        "got Int",
    );
}

#[test]
fn included_module_methods_count_for_structural_interface() {
    typecheck_ok(
        r#"
interface Greeter {
  def greet -> String
}

module Greeting {
  def greet -> String { "hi" }
}

class Person {
  include(Greeting)
}

def greet(person: Greeter) -> String {
  person.greet()
}

greet(Person.new())
"#,
    );
}
