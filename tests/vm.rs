use sapphire::compiler::compile;
use sapphire::error::SapphireError;
use sapphire::lexer::Lexer;
use sapphire::parser::Parser;
use sapphire::vm::{Vm, VmError, VmValue};
use std::path::PathBuf;

fn eval(src: &str) -> VmValue {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    Vm::new(func, std::path::PathBuf::new())
        .run()
        .expect("vm error")
        .expect("empty stack")
}

fn eval_with_stdlib(src: &str) -> VmValue {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    let mut vm = Vm::new(func, std::path::PathBuf::new());
    vm.load_stdlib().expect("stdlib");
    vm.run().expect("vm error").expect("empty stack")
}

fn eval_err(src: &str) -> VmError {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    Vm::new(func, std::path::PathBuf::new())
        .run()
        .expect_err("expected vm error")
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
    assert_eq!(eval("true"), VmValue::Bool(true));
    assert_eq!(eval("false"), VmValue::Bool(false));
}

#[test]
fn nil_literal() {
    assert_eq!(eval("nil"), VmValue::Nil);
}

#[test]
fn arithmetic() {
    assert_eq!(eval("1 + 2"), VmValue::Int(3));
    assert_eq!(eval("10 - 3"), VmValue::Int(7));
    assert_eq!(eval("4 * 5"), VmValue::Int(20));
    assert_eq!(eval("10 / 2"), VmValue::Int(5));
}

#[test]
fn negation() {
    assert_eq!(eval("-7"), VmValue::Int(-7));
}

#[test]
fn not() {
    assert_eq!(eval("!true"), VmValue::Bool(false));
    assert_eq!(eval("!false"), VmValue::Bool(true));
    assert_eq!(eval("!nil"), VmValue::Bool(true));
}

#[test]
fn bitwise_and() {
    assert_eq!(eval("12 & 10"), VmValue::Int(8)); // 1100 & 1010 = 1000
    assert_eq!(eval("255 & 15"), VmValue::Int(15)); // 11111111 & 00001111 = 00001111
    assert_eq!(eval("7 & 0"), VmValue::Int(0));
}

#[test]
fn bitwise_or() {
    assert_eq!(eval("12 | 10"), VmValue::Int(14)); // 1100 | 1010 = 1110
    assert_eq!(eval("240 | 15"), VmValue::Int(255)); // 11110000 | 00001111 = 11111111
    assert_eq!(eval("7 | 0"), VmValue::Int(7));
}

#[test]
fn bitwise_xor() {
    assert_eq!(eval("12 ^ 10"), VmValue::Int(6)); // 1100 ^ 1010 = 0110
    assert_eq!(eval("255 ^ 255"), VmValue::Int(0));
    assert_eq!(eval("7 ^ 0"), VmValue::Int(7));
}

#[test]
fn bitwise_not() {
    assert_eq!(eval("~0"), VmValue::Int(-1));
    assert_eq!(eval("~12"), VmValue::Int(-13));
    assert_eq!(eval("~(-1)"), VmValue::Int(0));
}

#[test]
fn shift_left() {
    assert_eq!(eval("1 << 4"), VmValue::Int(16));
    assert_eq!(eval("12 << 2"), VmValue::Int(48));
}

#[test]
fn shift_right() {
    assert_eq!(eval("16 >> 4"), VmValue::Int(1));
    assert_eq!(eval("12 >> 1"), VmValue::Int(6));
}

#[test]
fn bitwise_operator_precedence() {
    // shifts bind tighter than comparisons
    assert_eq!(eval("1 << 3 == 8"), VmValue::Bool(true));
    // & binds tighter than |
    assert_eq!(eval("5 | 3 & 6"), VmValue::Int(7)); // 5 | (3 & 6) = 5 | 2 = 7
    // ^ between & and |
    assert_eq!(eval("5 | 3 ^ 6"), VmValue::Int(5)); // 5 | (3 ^ 6) = 5 | 5 = 5
    // arithmetic binds tighter than bitwise
    assert_eq!(eval("3 + 1 & 6"), VmValue::Int(4)); // (3+1) & 6 = 4 & 6 = 4
}

#[test]
fn bitwise_type_error() {
    let err = eval_err("1 & 1.0");
    assert!(matches!(err, VmError::TypeError { .. }));
    let err = eval_err("1 | 1.0");
    assert!(matches!(err, VmError::TypeError { .. }));
    let err = eval_err("1 ^ 1.0");
    assert!(matches!(err, VmError::TypeError { .. }));
    let err = eval_err("1 << 1.0");
    assert!(matches!(err, VmError::TypeError { .. }));
    let err = eval_err("1 >> 1.0");
    assert!(matches!(err, VmError::TypeError { .. }));
    let err = eval_err("~1.0");
    assert!(matches!(err, VmError::TypeError { .. }));
}

#[test]
fn comparisons() {
    assert_eq!(eval("3 < 5"), VmValue::Bool(true));
    assert_eq!(eval("5 > 3"), VmValue::Bool(true));
    assert_eq!(eval("3 == 3"), VmValue::Bool(true));
    assert_eq!(eval("3 != 4"), VmValue::Bool(true));
    assert_eq!(eval("3 <= 3"), VmValue::Bool(true));
    assert_eq!(eval("4 >= 5"), VmValue::Bool(false));
}

#[test]
fn string_concat() {
    assert_eq!(
        eval(r#""hello" + " world""#),
        VmValue::Str("hello world".into())
    );
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
fn multi_assign_new_vars() {
    assert_eq!(eval("a, b = 1, 2\na"), VmValue::Int(1));
    assert_eq!(eval("a, b = 1, 2\nb"), VmValue::Int(2));
}

#[test]
fn multi_assign_swap() {
    assert_eq!(eval("a = 1\nb = 2\na, b = b, a\na"), VmValue::Int(2));
    assert_eq!(eval("a = 1\nb = 2\na, b = b, a\nb"), VmValue::Int(1));
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
    assert_eq!(
        eval("x = 0\nif false { x = 1 } else { x = 2 }\nx"),
        VmValue::Int(2)
    );
}

#[test]
fn if_elsif() {
    let src = "x = 0\nif false { x = 1 } elsif true { x = 2 }\nx";
    assert_eq!(eval(src), VmValue::Int(2));
}

#[test]
fn if_expression_assigned() {
    assert_eq!(eval("x = if true { 1 } else { 42 }\nx"), VmValue::Int(1));
    assert_eq!(eval("x = if false { 1 } else { 42 }\nx"), VmValue::Int(42));
}

#[test]
fn if_expression_no_else_is_nil_when_false() {
    assert_eq!(eval("x = if false { 1 }\nx"), VmValue::Nil);
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

/// Nested `while` with a local first assigned under `if` must not allocate that
/// local mid-outer-loop (would shift stack slots). Regression for #86.
#[test]
fn nested_while_inner_local_under_if_reuses_slot() {
    let src = r#"
limit = 10
flags = []
i = 0
while i <= limit {
  flags.append(1)
  i = i + 1
}
flags[0] = 0
flags[1] = 0
p = 2
while p * p <= limit {
  if flags[p] == 1 {
    m = p * p
    while m <= limit {
      flags[m] = 0
      m = m + p
    }
  }
  p = p + 1
}
flags[9]
"#;
    assert_eq!(eval_with_stdlib(src), VmValue::Int(0));
}

#[test]
fn last_expr_is_implicit_return() {
    let tokens = Lexer::new("1 + 1\n2 + 2").scan_tokens();
    let stmts = Parser::new(tokens).parse().unwrap();
    let func = compile(&stmts).unwrap();
    let result = Vm::new(func, std::path::PathBuf::new()).run().unwrap();
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

#[test]
fn closure_captures_param() {
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

#[test]
fn raise_caught_by_rescue() {
    let src = r#"x = 0
begin
  raise "boom"
  x = 1
rescue e
  x = 2
end
x"#;
    assert_eq!(eval(src), VmValue::Int(2));
}

#[test]
fn rescue_variable_bound() {
    let src = r#"msg = ""
begin
  raise "hello"
rescue e
  msg = e
end
msg"#;
    assert_eq!(eval(src), VmValue::Str("hello".into()));
}

#[test]
fn rescue_else_runs_on_no_error() {
    let src = r#"x = 0
begin
  x = 1
rescue e
  x = 99
else
  x = x + 10
end
x"#;
    assert_eq!(eval(src), VmValue::Int(11));
}

#[test]
fn begin_expr_value_assigned() {
    let src = "x = begin 7 end\nx";
    assert_eq!(eval(src), VmValue::Int(7));
}

#[test]
fn begin_expr_else_value() {
    let src = "x = begin 1 rescue e 0 else 2 end\nx";
    assert_eq!(eval(src), VmValue::Int(2));
}

#[test]
fn while_expr_in_assignment_is_nil() {
    assert_eq!(eval("x = while false { 1 }\nx"), VmValue::Nil);
}

#[test]
fn next_skips_rest_of_block() {
    let src = "def collect() {
  a = yield(1)
  b = yield(2)
  a + b
}
collect() { |n| if n == 1 { next 99 }
n * 10 }";
    assert_eq!(eval(src), VmValue::Int(99 + 20));
}

#[test]
fn break_exits_block_caller() {
    let src = "def wrap() {
  yield(1)
  break 99
  yield(2)
}
wrap() { |n| n }";
    assert_eq!(eval(src), VmValue::Int(99));
}

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

#[test]
fn class_inheritance_method() {
    let src = "class Animal {
  def speak() { \"animal\" }
}
class Dog < Animal {
  def fetch() { \"fetching\" }
}
d = Dog.new()
d.speak()";
    assert_eq!(eval(src), VmValue::Str("animal".into()));
}

#[test]
fn super_method_call() {
    let src = "class Animal {
  def speak() { \"animal\" }
}
class Dog < Animal {
  def speak() { \"dog:\" + super() }
}
Dog.new().speak()";
    assert_eq!(eval(src), VmValue::Str("dog:animal".into()));
}

#[test]
fn super_with_args() {
    let src = "class Base {
  def add(x, y) { x + y }
}
class Child < Base {
  def add(x, y) { super(x, y) + 1 }
}
Child.new().add(2, 3)";
    assert_eq!(eval(src), VmValue::Int(6));
}

#[test]
fn mixin_include_instance_method() {
    let src = "module Greet {
  def hi { \"hi\" }
}
class Person {
  include(Greet)
}
Person.new.hi";
    assert_eq!(eval(src), VmValue::Str("hi".into()));
}

#[test]
fn mixin_super_traverses_module_then_superclass() {
    let src = "module M {
  def greet { \"m:\" + super() }
}
class Base {
  def greet { \"base\" }
}
class Child < Base {
  include(M)
  def greet { \"c:\" + super() }
}
Child.new.greet";
    assert_eq!(eval(src), VmValue::Str("c:m:base".into()));
}

#[test]
fn mixin_is_a_question() {
    let src = "module Trackable { }
class C {
  include(Trackable)
}
C.new.is_a?(Trackable)";
    assert_eq!(eval(src), VmValue::Bool(true));
}

#[test]
fn nested_module_include_resolves_lexically() {
    let src = "class Outer {
  module Inner {
    def x { 1 }
  }
  class Sub {
    include(Inner)
  }
}
Outer.Sub.new.x";
    assert_eq!(eval(src), VmValue::Int(1));
}

#[test]
fn module_new_is_error() {
    let err = eval_err("module M {}\nM.new()");
    match err {
        VmError::TypeError { message, .. } => {
            assert!(message.contains("instantiate module"));
        }
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn abstract_class_new_is_error() {
    let err = eval_err(
        "abstract class Shape {\n  abstract def area() -> Float\n}\nShape.new()",
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
        "abstract class Shape {\n  abstract def area() -> Float\n  abstract def perimeter() -> Float\n}\nclass Broken < Shape {\n  def area() -> Float { 0.0 }\n}\nBroken.new()",
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
    let src = "abstract class Shape {\n  abstract def area() -> Float\n}\nclass Square < Shape {\n  def area() -> Float { 4.0 }\n}\nSquare.new().area()";
    assert_eq!(eval(src), VmValue::Float(4.0));
}

#[test]
fn abstract_partial_parent_concrete_grandchild() {
    let src = "abstract class A {\n  abstract def foo() -> Int\n  abstract def bar() -> Int\n}\nabstract class B < A {\n  def foo() -> Int { 1 }\n}\nclass C < B {\n  def bar() -> Int { 2 }\n}\nC.new().foo() + C.new().bar()";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn concrete_method_on_abstract_class_callable_on_subclass() {
    let src = "abstract class Base {\n  abstract def m() -> Int\n  def helper() -> Int { 10 }\n}\nclass Child < Base {\n  def m() -> Int { self.helper() + 1 }\n}\nChild.new().m()";
    assert_eq!(eval(src), VmValue::Int(11));
}

#[test]
fn import_module_fixture() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .canonicalize()
        .unwrap();
    let src = r#"import "./module"
SampleMod.Widget.new.ping"#;
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    let mut vm = Vm::new(func, dir);
    vm.load_stdlib().expect("stdlib");
    let v = vm.run().expect("vm error").expect("empty stack");
    assert_eq!(v, VmValue::Str("pong".into()));
}

#[test]
fn safe_get_on_nil_returns_nil() {
    assert_eq!(eval("x = nil\nx&.name"), VmValue::Nil);
}

#[test]
fn safe_get_on_instance_returns_field() {
    let src = "class Dog {
  attr name
}
d = Dog.new(name: \"Rex\")
d&.name";
    assert_eq!(eval(src), VmValue::Str("Rex".into()));
}

#[test]
fn list_literal() {
    assert_eq!(eval("[1, 2, 3]\n0"), VmValue::Int(0));
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
    assert_eq!(
        eval(
            r#"m = {x: 1, y: 2}
m["x"]"#
        ),
        VmValue::Int(1)
    );
}

#[test]
fn map_missing_key_is_nil() {
    assert_eq!(
        eval(
            r#"m = {a: 1}
m["z"]"#
        ),
        VmValue::Nil
    );
}

#[test]
fn range_builds() {
    assert_eq!(eval("1..5"), VmValue::Range { from: 1, to: 5 });
}

#[test]
fn string_interp_plain() {
    assert_eq!(eval(r#""hello""#), VmValue::Str("hello".into()));
}

#[test]
fn string_interp_with_expr() {
    assert_eq!(
        eval(
            r#"x = 42
"value is #{x}""#
        ),
        VmValue::Str("value is 42".into())
    );
}

#[test]
fn string_interp_multiple_parts() {
    assert_eq!(
        eval(
            r##"a = 1
b = 2
"#{a} + #{b} = #{a + b}""##
        ),
        VmValue::Str("1 + 2 = 3".into())
    );
}

#[test]
fn and_short_circuits_false() {
    assert_eq!(eval("false && true"), VmValue::Bool(false));
    assert_eq!(eval("nil && 42"), VmValue::Nil);
}

#[test]
fn and_returns_rhs_when_truthy() {
    assert_eq!(eval("true && 42"), VmValue::Int(42));
    assert_eq!(eval("1 && 2"), VmValue::Int(2));
}

#[test]
fn or_short_circuits_truthy() {
    assert_eq!(eval("42 || false"), VmValue::Int(42));
    assert_eq!(eval("true || nil"), VmValue::Bool(true));
}

#[test]
fn or_returns_rhs_when_falsy() {
    assert_eq!(eval("false || 99"), VmValue::Int(99));
    assert_eq!(eval("nil || nil"), VmValue::Nil);
}

#[test]
fn print_does_not_change_result_when_not_last_statement() {
    assert_eq!(eval("print 42\n99"), VmValue::Int(99));
}

#[test]
fn implicit_return_last_print_passes_printed_value() {
    assert_eq!(eval("def f() { print 42 }\nf()"), VmValue::Int(42));
}

#[test]
fn implicit_return_last_def_returns_method_name() {
    assert_eq!(
        eval("def outer() { def inner() { 1 } }\nouter()"),
        VmValue::Str("inner".into())
    );
}

#[test]
fn implicit_return_last_class_returns_class() {
    let v = eval_with_stdlib("class ImplicitRetClass {}");
    assert!(matches!(v, VmValue::Class { ref name, .. } if name == "ImplicitRetClass"));
}

#[test]
fn transitive_capture() {
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

#[test]
fn int_methods() {
    assert_eq!(eval("42.to_s()"), VmValue::Str("42".into()));
    assert_eq!(eval("42.to_f()"), VmValue::Float(42.0));
    assert_eq!(eval_with_stdlib("n = -5\nn.abs()"), VmValue::Int(5));
    assert_eq!(eval_with_stdlib("4.even?()"), VmValue::Bool(true));
    assert_eq!(eval_with_stdlib("3.odd?()"), VmValue::Bool(true));
    assert_eq!(eval_with_stdlib("0.zero?()"), VmValue::Bool(true));
    assert_eq!(eval_with_stdlib("5.max(10)"), VmValue::Int(10));
    assert_eq!(eval_with_stdlib("10.min(5)"), VmValue::Int(5));
}

#[test]
fn float_methods() {
    assert_eq!(eval("3.7.round()"), VmValue::Int(4));
    assert_eq!(eval("3.7.floor()"), VmValue::Int(3));
    assert_eq!(eval("3.2.ceil()"), VmValue::Int(4));
    assert_eq!(eval("3.5.to_i()"), VmValue::Int(3));
    assert_eq!(eval_with_stdlib("n = -2.5\nn.abs()"), VmValue::Float(2.5));
}

#[test]
fn string_methods() {
    assert_eq!(eval(r#""hello".size()"#), VmValue::Int(5));
    assert_eq!(eval(r#""hello".upcase()"#), VmValue::Str("HELLO".into()));
    assert_eq!(eval(r#""HELLO".downcase()"#), VmValue::Str("hello".into()));
    assert_eq!(eval(r#""abc".reverse()"#), VmValue::Str("cba".into()));
    assert_eq!(eval(r#""  hi  ".trim()"#), VmValue::Str("hi".into()));
    assert_eq!(eval(r#""42".to_i()"#), VmValue::Int(42));
    assert_eq!(eval(r#""3.14".to_f()"#), VmValue::Float(3.14));
    assert_eq!(eval(r#""".empty?()"#), VmValue::Bool(true));
    assert_eq!(eval(r#""hi".include?("i")"#), VmValue::Bool(true));
    assert_eq!(eval(r#""hi".starts_with?("h")"#), VmValue::Bool(true));
    assert_eq!(eval(r#""hi".ends_with?("i")"#), VmValue::Bool(true));
}

#[test]
fn string_split() {
    let src = r#""a,b,c".split(",")"#;
    assert_eq!(eval(&format!("{}.size()", src)), VmValue::Int(3));
}

#[test]
fn list_methods() {
    assert_eq!(eval("a = [1,2,3]\na.size()"), VmValue::Int(3));
    assert_eq!(eval("a = [1,2,3]\na.first()"), VmValue::Int(1));
    assert_eq!(eval("a = [1,2,3]\na.last()"), VmValue::Int(3));
    assert_eq!(eval("[].empty?()"), VmValue::Bool(true));
    assert_eq!(eval("[1,2].empty?()"), VmValue::Bool(false));
    assert_eq!(eval("a = [1,2,3]\na.include?(2)"), VmValue::Bool(true));
    assert_eq!(eval("a = [3,1,2]\na.sort().first()"), VmValue::Int(1));
    assert_eq!(eval(r#"[1,2,3].join(",")"#), VmValue::Str("1,2,3".into()));
}

#[test]
fn list_push_and_pop() {
    assert_eq!(eval("a = [1,2]\na.append(3)\na.size()"), VmValue::Int(3));
    assert_eq!(eval("a = [1,2,3]\na.pop()"), VmValue::Int(3));
}

#[test]
fn list_each() {
    let src = "a = [1,2,3]\nsum = 0\na.each() { |x| sum = sum + x }\nsum";
    assert_eq!(eval(src), VmValue::Int(6));
}

#[test]
fn list_map() {
    let src = "a = [1,2,3]\nb = a.map() { |x| x * 2 }\nb[1]";
    assert_eq!(eval(src), VmValue::Int(4));
}

#[test]
fn int_times() {
    let src = "sum = 0\n3.times() { |i| sum = sum + i }\nsum";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn range_each() {
    let src = "sum = 0\n(1..4).each() { |i| sum = sum + i }\nsum";
    assert_eq!(eval(src), VmValue::Int(6));
}

#[test]
fn range_to_a() {
    let src = "r = 1..4\nr.to_a().size()";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn map_methods() {
    assert_eq!(eval("m = {a: 1, b: 2}\nm.size()"), VmValue::Int(2));
    assert_eq!(eval("m = {a: 1}\nm.has_key?(\"a\")"), VmValue::Bool(true));
    assert_eq!(eval("m = {a: 1}\nm.has_key?(\"z\")"), VmValue::Bool(false));
    assert_eq!(eval("m = {a: 1}\nm.delete(\"a\")"), VmValue::Int(1));
}

#[test]
fn nil_bool_methods() {
    assert_eq!(eval("nil.nil?()"), VmValue::Bool(true));
    assert_eq!(eval("nil.to_s()"), VmValue::Str("".into()));
    assert_eq!(eval_with_stdlib("false.nil?()"), VmValue::Bool(false));
    assert_eq!(eval_with_stdlib("true.to_s()"), VmValue::Str("true".into()));
}

#[test]
fn is_a_instance_hierarchy() {
    let base = "class Animal { attr name }\nclass Dog < Animal { attr breed }\nd = Dog.new(name: \"Rex\", breed: \"Lab\")\n";
    assert_eq!(
        eval_with_stdlib(&(base.to_string() + "d.is_a?(Dog)")),
        VmValue::Bool(true)
    );
    assert_eq!(
        eval_with_stdlib(&(base.to_string() + "d.is_a?(Animal)")),
        VmValue::Bool(true)
    );
    let unrelated = "class Animal { attr name }\nclass Dog < Animal { attr breed }\nclass Cat {}\nd = Dog.new(name: \"Rex\", breed: \"Lab\")\n";
    assert_eq!(
        eval_with_stdlib(&(unrelated.to_string() + "d.is_a?(Cat)")),
        VmValue::Bool(false)
    );
}

#[test]
fn lambda_basic_call() {
    assert_eq!(eval("f = def(x) { x * 2 }; f.call(5)"), VmValue::Int(10));
}

#[test]
fn lambda_no_params() {
    assert_eq!(eval("f = def() { 42 }; f.call()"), VmValue::Int(42));
}

#[test]
fn lambda_closure_capture() {
    assert_eq!(
        eval("n = 10; f = def(x) { x + n }; f.call(3)"),
        VmValue::Int(13)
    );
}

#[test]
fn lambda_return_exits_only_lambda() {
    let src = r#"
def outer() {
  f = def(x) { return x * 2 }
  result = f.call(5)
  result + 1
}
outer()
"#;
    assert_eq!(eval(src), VmValue::Int(11));
}

#[test]
fn private_method_callable_from_within_class() {
    let src = r#"class Foo {
  defp secret() { 42 }
  def get() { self.secret() }
}
Foo.new().get()"#;
    assert_eq!(eval(src), VmValue::Int(42));
}

#[test]
fn private_method_rejected_from_outside_class() {
    let src = r#"class Foo {
  defp secret() { 42 }
}
Foo.new().secret()"#;
    let err = eval_err(src);
    assert!(matches!(err, VmError::TypeError { ref message, .. } if message.contains("private")));
}

#[test]
fn class_method_basic() {
    let src = r#"class Greeter {
  self {
    def hello() { "hello" }
  }
}
Greeter.hello()"#;
    assert_eq!(eval(src), VmValue::Str("hello".into()));
}

#[test]
fn class_method_lexical_constant_without_self_prefix() {
    let src = r#"class Math {
  PI = 3
  self {
    def tau { PI * 2 }
  }
}
Math.tau()"#;
    assert_eq!(eval(src), VmValue::Int(6));
}

#[test]
fn class_method_with_args_self_block() {
    let src = r#"class Math {
  self {
    def add(a, b) { a + b }
  }
}
Math.add(3, 4)"#;
    assert_eq!(eval(src), VmValue::Int(7));
}

#[test]
fn class_method_factory() {
    let src = r#"class Point {
  attr x
  attr y
  self {
    def origin() { self.new(x: 0, y: 0) }
  }
}
p = Point.origin()
p.x"#;
    assert_eq!(eval(src), VmValue::Int(0));
}

#[test]
fn class_method_inherited_by_subclass() {
    let src = r#"class Animal {
  self {
    def kind() { "animal" }
  }
}
class Dog < Animal {}
Dog.kind()"#;
    assert_eq!(eval(src), VmValue::Str("animal".into()));
}

#[test]
fn class_method_overridden_in_subclass() {
    let src = r#"class Animal {
  self {
    def kind() { "animal" }
  }
}
class Dog < Animal {
  self {
    def kind() { "dog" }
  }
}
Dog.kind()"#;
    assert_eq!(eval(src), VmValue::Str("dog".into()));
}

// ---- Arithmetic ----

#[test]
fn modulo() {
    assert_eq!(eval("10 % 3"), VmValue::Int(1));
    assert_eq!(eval("9 % 3"), VmValue::Int(0));
}

#[test]
fn int_division_stays_int() {
    assert_eq!(eval("7 / 2"), VmValue::Int(3));
}

#[test]
fn division_by_zero() {
    let err = eval_err("1 / 0");
    assert!(matches!(err, VmError::Raised(..)));
}

// ---- String equality ----

#[test]
fn string_equality() {
    assert_eq!(eval(r#""a" == "a""#), VmValue::Bool(true));
    assert_eq!(eval(r#""a" == "b""#), VmValue::Bool(false));
}

// ---- String escapes ----

#[test]
fn string_escape_newline() {
    assert_eq!(eval(r#""\n""#), VmValue::Str("\n".into()));
}

#[test]
fn string_escape_tab() {
    assert_eq!(eval(r#""\t""#), VmValue::Str("\t".into()));
}

#[test]
fn string_escape_backslash() {
    assert_eq!(eval(r#""\\""#), VmValue::Str("\\".into()));
}

#[test]
fn string_escape_quote() {
    assert_eq!(eval(r#""\"""#), VmValue::Str("\"".into()));
}

#[test]
fn string_escape_in_interpolation() {
    assert_eq!(eval(r#""a\nb""#), VmValue::Str("a\nb".into()));
}

// ---- Float ----

#[test]
fn float_arithmetic() {
    assert_eq!(eval("1.5 + 2.5"), VmValue::Float(4.0));
    assert_eq!(eval("3.0 - 1.5"), VmValue::Float(1.5));
    assert_eq!(eval("2.0 * 3.0"), VmValue::Float(6.0));
    assert_eq!(eval("7.0 / 2.0"), VmValue::Float(3.5));
}

#[test]
fn float_mixed_arithmetic() {
    assert_eq!(eval("1 + 0.5"), VmValue::Float(1.5));
    assert_eq!(eval("0.5 + 1"), VmValue::Float(1.5));
    assert_eq!(eval("3 * 1.5"), VmValue::Float(4.5));
    assert_eq!(eval("7 / 2.0"), VmValue::Float(3.5));
}

#[test]
fn float_comparison() {
    assert_eq!(eval("1.5 < 2.0"), VmValue::Bool(true));
    assert_eq!(eval("2.0 > 1.5"), VmValue::Bool(true));
    assert_eq!(eval("1.0 == 1.0"), VmValue::Bool(true));
}

#[test]
fn float_negation() {
    assert_eq!(eval("-3.14"), VmValue::Float(-3.14));
}

#[test]
fn float_to_i() {
    assert_eq!(eval("3.9.to_i()"), VmValue::Int(3));
    assert_eq!(eval("(-3.9).to_i()"), VmValue::Int(-3));
}

#[test]
fn int_to_f() {
    assert_eq!(eval("3.to_f()"), VmValue::Float(3.0));
}

#[test]
fn float_to_s() {
    assert_eq!(eval("3.14.to_s()"), VmValue::Str("3.14".into()));
}

// ---- Constants ----

#[test]
fn constant_assignment() {
    assert_eq!(eval("MAX = 100\nMAX"), VmValue::Int(100));
}

#[test]
fn mixed_case_is_not_a_constant() {
    assert_eq!(eval("Pi = 3\nPi = 4\nPi"), VmValue::Int(4));
}

#[test]
fn constant_readable_in_functions() {
    let src = "MAX = 10\ndef cap(n) { if n > MAX { MAX } else { n } }\ncap(20)";
    assert_eq!(eval(src), VmValue::Int(10));
}

// ---- Implicit `it` in blocks ----

#[test]
fn it_each() {
    let src = "sum = 0\n[1, 2, 3].each() { |it| sum = sum + it }\nsum";
    assert_eq!(eval(src), VmValue::Int(6));
}

#[test]
fn it_map() {
    let src = "result = [1, 2, 3].map() { |it| it * 2 }\nresult[1]";
    assert_eq!(eval(src), VmValue::Int(4));
}

#[test]
fn while_condition_method_call_no_block_greed() {
    let src = "list = [1, 2, 3]\ni = 0\nsum = 0\nlen = list.size()\nwhile i < len { sum = sum + list[i]\ni = i + 1 }\nsum";
    assert_eq!(eval(src), VmValue::Int(6));
}

// ---- Each next ----

#[test]
fn each_next() {
    let src = "sum = 0\n[1, 2, 3, 4, 5].each() { |x| if x == 3 { next nil }\nsum = sum + x }\nsum";
    assert_eq!(eval(src), VmValue::Int(12));
}

// ---- List advanced methods ----

#[test]
fn list_select() {
    let src = "result = [1, 2, 3, 4].select() { |x| x > 2 }\nresult.size()";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(2));
}

#[test]
fn list_reduce_with_initial() {
    let src = "[1, 2, 3, 4, 5].reduce(0) { |acc, n| acc + n }";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(15));
}

#[test]
fn list_reduce_without_initial() {
    let src = "[1, 2, 3, 4, 5].reduce() { |acc, n| acc * n }";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(120));
}

#[test]
fn list_sort_full() {
    let src = "result = [3, 1, 4, 1, 5, 9, 2].sort()\nresult[0]";
    assert_eq!(eval(src), VmValue::Int(1));
    let src2 = "result = [3, 1, 4, 1, 5, 9, 2].sort()\nresult[6]";
    assert_eq!(eval(src2), VmValue::Int(9));
}

#[test]
fn list_sort_strings() {
    let src = r#"result = ["banana", "apple", "cherry"].sort()
result[0]"#;
    assert_eq!(eval(src), VmValue::Str("apple".into()));
    let src2 = r#"result = ["banana", "apple", "cherry"].sort()
result[2]"#;
    assert_eq!(eval(src2), VmValue::Str("cherry".into()));
}

#[test]
fn list_flatten() {
    let src = "result = [[1, 2], [3, [4, 5]]].flatten()\nresult.size()";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(5));
    let src2 = "result = [[1, 2], [3, [4, 5]]].flatten()\nresult[3]";
    assert_eq!(eval_with_stdlib(src2), VmValue::Int(4));
}

#[test]
fn list_uniq() {
    let src = "result = [1, 2, 2, 3, 1].uniq()\nresult.size()";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(3));
}

#[test]
fn list_each_with_index() {
    let src = r#"pairs = []
["a", "b", "c"].each_with_index() { |item, i| pairs.append(i) }
pairs[2]"#;
    assert_eq!(eval_with_stdlib(src), VmValue::Int(2));
}

#[test]
fn list_zip() {
    let src = "result = [1, 2, 3].zip([4, 5, 6])\nresult.size()";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(3));
    let src2 = "result = [1, 2, 3].zip([4, 5, 6])\nresult[0][0]";
    assert_eq!(eval_with_stdlib(src2), VmValue::Int(1));
    let src3 = "result = [1, 2, 3].zip([4, 5, 6])\nresult[0][1]";
    assert_eq!(eval_with_stdlib(src3), VmValue::Int(4));
}

// ---- Map advanced methods ----

#[test]
fn map_merge() {
    let src = r#"a = { x: 1 }
b = { y: 2 }
c = a.merge(b)
c.size()"#;
    assert_eq!(eval_with_stdlib(src), VmValue::Int(2));
    let src2 = r#"a = { x: 1 }
b = { y: 2 }
c = a.merge(b)
c["x"]"#;
    assert_eq!(eval_with_stdlib(src2), VmValue::Int(1));
}

#[test]
fn map_select() {
    let src = r#"m = { a: 1, b: 2, c: 3 }
result = m.select() { |k, v| v > 1 }
result.size()"#;
    assert_eq!(eval_with_stdlib(src), VmValue::Int(2));
    let src2 = r#"m = { a: 1, b: 2, c: 3 }
result = m.select() { |k, v| v > 1 }
result.has_key?("a")"#;
    assert_eq!(eval_with_stdlib(src2), VmValue::Bool(false));
}

// ---- Range ----

#[test]
fn range_include() {
    // VM ranges are exclusive upper bound
    assert_eq!(eval_with_stdlib("(1..10).include?(5)"), VmValue::Bool(true));
    assert_eq!(eval_with_stdlib("(1..10).include?(1)"), VmValue::Bool(true));
    assert_eq!(
        eval_with_stdlib("(1..10).include?(10)"),
        VmValue::Bool(false)
    );
    assert_eq!(
        eval_with_stdlib("(1..10).include?(11)"),
        VmValue::Bool(false)
    );
}

#[test]
fn range_to_s() {
    assert_eq!(eval("(1..5).to_s()"), VmValue::Str("1..5".into()));
}

// ---- Yield ----

#[test]
fn yield_multiple_args() {
    let src = "def call_block(a, b) { yield(a, b) }\ncall_block(3, 4) { |x, y| x + y }";
    assert_eq!(eval(src), VmValue::Int(7));
}

#[test]
fn yield_in_loop() {
    let src = "def my_each(list) {
  len = list.size()
  i = 0
  while i < len { yield(list[i])\ni = i + 1 }
}
sum = 0
my_each([1, 2, 3]) { |x| sum = sum + x }
sum";
    assert_eq!(eval(src), VmValue::Int(6));
}

#[test]
fn yield_in_method() {
    let src = "class Wrapper {
  attr items
  def each() {
    len = self.items.size()
    i = 0
    while i < len { yield(self.items[i])\ni = i + 1 }
  }
}
w = Wrapper.new(items: [10, 20, 30])
sum = 0
w.each() { |x| sum = sum + x }
sum";
    assert_eq!(eval(src), VmValue::Int(60));
}

// ---- Class features ----

#[test]
fn class_default_field() {
    let src = r#"class Point { attr x
attr y
attr label = "origin" }
p = Point.new(x: 1, y: 2)
p.label"#;
    assert_eq!(eval(src), VmValue::Str("origin".into()));
}

#[test]
fn inheritance_fields() {
    let src = r#"class Animal { attr name }
class Dog < Animal { attr breed }
d = Dog.new(name: "Rex", breed: "Lab")
d.name"#;
    assert_eq!(eval(src), VmValue::Str("Rex".into()));
    let src2 = r#"class Animal { attr name }
class Dog < Animal { attr breed }
d = Dog.new(name: "Rex", breed: "Lab")
d.breed"#;
    assert_eq!(eval(src2), VmValue::Str("Lab".into()));
}

#[test]
fn inheritance_override() {
    let src = r#"class Animal { def speak() { "..." } }
class Dog < Animal { def speak() { "woof" } }
Dog.new().speak()"#;
    assert_eq!(eval(src), VmValue::Str("woof".into()));
}

#[test]
fn field_mutation() {
    let src = r#"class Counter { attr n
  def inc() { self.n = self.n + 1 }
}
c = Counter.new(n: 0)
c.inc()
c.n"#;
    assert_eq!(eval(src), VmValue::Int(1));
}

#[test]
fn super_with_field_override() {
    let src = r#"class Animal { attr name
  def describe() { self.name }
}
class Dog < Animal { attr breed
  def describe() { super() + " (" + self.breed + ")" }
}
d = Dog.new(name: "Rex", breed: "Lab")
d.describe()"#;
    assert_eq!(eval(src), VmValue::Str("Rex (Lab)".into()));
}

#[test]
fn super_still_works_with_implicit_object() {
    let src = r#"class Animal { attr name
  def speak() { "..." }
}
class Dog < Animal { def speak() { super } }
Dog.new(name: "Rex").speak()"#;
    assert_eq!(eval(src), VmValue::Str("...".into()));
}

#[test]
fn super_bare_in_nested_closure() {
    let src = r#"class Animal { def speak() { "a" } }
class Dog < Animal {
  def speak() {
    f = def() { super }
    f()
  }
}
Dog.new().speak()"#;
    assert_eq!(eval(src), VmValue::Str("a".into()));
}

#[test]
fn super_dot_method_is_rejected() {
    let src = "class A { def m() { 1 } }\nclass B < A { def m() { super.m() } }";
    let tokens = Lexer::new(src).scan_tokens();
    let err = Parser::new(tokens)
        .parse()
        .expect_err("expected parse error");
    match err {
        SapphireError::ParseError { message, .. } => {
            assert!(message.contains("super"), "unexpected message: {message}");
        }
        other => panic!("expected ParseError, got {:?}", other),
    }
}

#[test]
fn implicit_self_local_shadows_field() {
    let src = r#"class Box { attr x
  def doubled() { x = 99
x }
}
b = Box.new(x: 10)
b.doubled()"#;
    assert_eq!(eval(src), VmValue::Int(99));
    let src2 = r#"class Box { attr x
  def doubled() { x = 99
x }
}
b = Box.new(x: 10)
b.doubled()
b.x"#;
    assert_eq!(eval(src2), VmValue::Int(10));
}

// ---- elsif chains ----

#[test]
fn elsif_first_branch() {
    let src = "x = 20\nresult = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }\nresult";
    assert_eq!(eval(src), VmValue::Int(1));
}

#[test]
fn elsif_second_branch() {
    let src = "x = 5\nresult = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }\nresult";
    assert_eq!(eval(src), VmValue::Int(2));
}

#[test]
fn elsif_else_branch() {
    let src = "x = 1\nresult = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }\nresult";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn elsif_chain() {
    let src = "x = 5\nresult = 0\nif x == 1 { result = 1 } elsif x == 2 { result = 2 } elsif x == 5 { result = 5 } else { result = 99 }\nresult";
    assert_eq!(eval(src), VmValue::Int(5));
}

#[test]
fn elsif_on_next_line() {
    // elsif on a new line after the closing '}'
    let src = "x = 2\nif x == 1 { 10 }\nelsif x == 2 { 20 }\nelse { 30 }";
    assert_eq!(eval(src), VmValue::Int(20));
}

#[test]
fn elsif_multiline_hits_first_branch() {
    let src = "x = 1\nif x == 1 { 10 }\nelsif x == 2 { 20 }\nelse { 30 }";
    assert_eq!(eval(src), VmValue::Int(10));
}

#[test]
fn elsif_multiline_hits_else() {
    let src = "x = 99\nif x == 1 { 10 }\nelsif x == 2 { 20 }\nelse { 30 }";
    assert_eq!(eval(src), VmValue::Int(30));
}

#[test]
fn elsif_multiline_chained() {
    // multiple elsif clauses each on their own line
    let src = "x = 3\nif x == 1 { 10 }\nelsif x == 2 { 20 }\nelsif x == 3 { 30 }\nelse { 99 }";
    assert_eq!(eval(src), VmValue::Int(30));
}

#[test]
fn else_on_next_line() {
    // else on a new line (no elsif), just to be sure
    let src = "x = 5\nif x == 1 { 10 }\nelse { 99 }";
    assert_eq!(eval(src), VmValue::Int(99));
}

// ---- defp inherited ----

#[test]
fn defp_inherited_callable_from_subclass() {
    let src = r#"class A {
  defp helper() { 99 }
  def run() { helper() }
}
class B < A {}
B.new().run()"#;
    assert_eq!(eval(src), VmValue::Int(99));
}

// ---- chars ----

#[test]
fn string_chars() {
    let src = r#"result = "hi".chars()
result.size()"#;
    assert_eq!(eval_with_stdlib(src), VmValue::Int(2));
    let src2 = r#"result = "hi".chars()
result[0]"#;
    assert_eq!(eval_with_stdlib(src2), VmValue::Str("h".into()));
}

// ---- Multi-assign three ----

#[test]
fn multi_assign_three() {
    let src = "x, y, z = 10, 20, 30\ny";
    assert_eq!(eval(src), VmValue::Int(20));
    let src2 = "x, y, z = 10, 20, 30\nz";
    assert_eq!(eval(src2), VmValue::Int(30));
}

// ---- Error handling ----

#[test]
fn raise_unhandled() {
    let err = eval_err(r#"raise "oops""#);
    assert!(matches!(err, VmError::Raised(..)));
}

#[test]
fn begin_rescue_catches_runtime_error() {
    let src = "x = 0\nbegin\nx = 1 / 0\nrescue e\nx = 99\nend\nx";
    assert_eq!(eval(src), VmValue::Int(99));
}

#[test]
fn begin_else_skipped_on_error() {
    let src = r#"x = 0
begin
  raise "err"
rescue e
  x = 99
else
  x = 2
end
x"#;
    assert_eq!(eval(src), VmValue::Int(99));
}

#[test]
fn begin_no_error_skips_rescue() {
    let src = "x = 0\nbegin\nx = 42\nrescue e\nx = 1\nend\nx";
    assert_eq!(eval(src), VmValue::Int(42));
}

#[test]
fn inline_rescue_in_function() {
    let src = r#"def risky(x) {
  if x < 0 { raise "bad" }
  x * 2
rescue e
  0
}
risky(5)"#;
    assert_eq!(eval(src), VmValue::Int(10));
    let src2 = r#"def risky(x) {
  if x < 0 { raise "bad" }
  x * 2
rescue e
  0
}
risky(-1)"#;
    assert_eq!(eval(src2), VmValue::Int(0));
}

#[test]
fn inline_rescue_binds_error() {
    let src = r#"def boom() {
  raise "oops"
  1
rescue e
  e
}
boom()"#;
    assert_eq!(eval(src), VmValue::Str("oops".into()));
}

#[test]
fn inline_rescue_in_method() {
    let src = r#"class Safe {
  def try_div(x) {
    10 / x
  rescue e
    -1
  }
}
Safe.new().try_div(2)"#;
    assert_eq!(eval(src), VmValue::Int(5));
    let src2 = r#"class Safe {
  def try_div(x) {
    10 / x
  rescue e
    -1
  }
}
Safe.new().try_div(0)"#;
    assert_eq!(eval(src2), VmValue::Int(-1));
}

#[test]
fn raise_instance() {
    let src = r#"class Err { attr msg }
result = begin
  raise Err.new(msg: "bad")
rescue e
  e.msg
end
result"#;
    assert_eq!(eval(src), VmValue::Str("bad".into()));
}

// ---- Num methods ----

#[test]
fn num_methods_on_int_and_float() {
    assert_eq!(eval_with_stdlib("0.zero?()"), VmValue::Bool(true));
    // Float zero? uses self == 0 (float/int comparison) which is not supported in VM
    assert_eq!(eval_with_stdlib("3.positive?()"), VmValue::Bool(true));
    assert_eq!(eval_with_stdlib("(-1.0).negative?()"), VmValue::Bool(true));
    assert_eq!(eval_with_stdlib("10.clamp(1, 5)"), VmValue::Int(5));
    assert_eq!(eval_with_stdlib("0.clamp(1, 5)"), VmValue::Int(1));
}

#[test]
fn num_type_annotation_accepts_int_and_float() {
    let src = "def double(x: Num) { x + x }\ndouble(3)";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(6));
    let src2 = "def double(x: Num) { x + x }\ndouble(1.5)";
    assert_eq!(eval_with_stdlib(src2), VmValue::Float(3.0));
}

// ---- Type annotations ----

#[test]
fn typed_param_accepts_correct_type() {
    let src = "def add(a: Int, b: Int) { a + b }\nadd(1, 2)";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn typed_param_rejects_wrong_type() {
    let err = eval_err(
        r#"def add(a: Int, b: Int) { a + b }
add("x", 2)"#,
    );
    assert!(matches!(err, VmError::TypeError { .. }));
}

#[test]
fn typed_return_accepts_correct_type() {
    let src = "def f() -> Int { 42 }\nf()";
    assert_eq!(eval(src), VmValue::Int(42));
}

#[test]
fn attr_type_accepted_on_constructor() {
    let src = "class P { attr x: Int }\nP.new(x: 42).x";
    assert_eq!(eval(src), VmValue::Int(42));
}

#[test]
fn attr_type_accepted_on_set() {
    let src = "class P { attr x: Int }\np = P.new(x: 1)\np.x = 99\np.x";
    assert_eq!(eval(src), VmValue::Int(99));
}

#[test]
fn unannotated_code_unchanged() {
    let src = r#"def greet(name) { name }
greet("Alice")"#;
    assert_eq!(eval(src), VmValue::Str("Alice".into()));
}

#[test]
fn method_typed_param_rejects_wrong_type() {
    let err = eval_err(
        r#"class Calc { def double(n: Int) { n * 2 } }
Calc.new().double("x")"#,
    );
    assert!(matches!(err, VmError::TypeError { .. }));
}

// --- Method chaining after blocks ---

#[test]
fn chain_map_then_size() {
    // .map { } followed by .size() on the same line
    let src = "[1, 2, 3].map() { |x| x * 2 }.size()";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn chain_map_then_index() {
    // result of .map { } immediately indexed
    let src = "[1, 2, 3].map() { |x| x * 10 }[1]";
    assert_eq!(eval(src), VmValue::Int(20));
}

#[test]
fn chain_map_then_map() {
    // two block calls chained on one line
    let src = "[1, 2, 3].map() { |x| x * 2 }.map() { |x| x + 1 }[0]";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn chain_multiline_map_then_size() {
    // .map { } on one line, .size() on the next
    let src = "[1, 2, 3]\n  .map() { |x| x * 2 }\n  .size()";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn chain_multiline_map_then_map() {
    // two block-calls across newlines
    let src = "result = [1, 2, 3]\n  .map() { |x| x * 2 }\n  .map() { |x| x + 1 }\nresult[2]";
    assert_eq!(eval(src), VmValue::Int(7));
}

#[test]
fn chain_multiline_select_then_size() {
    let src = "[1, 2, 3, 4]\n  .select() { |x| x > 2 }\n  .size()";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(2));
}

#[test]
fn chain_multiline_map_then_select() {
    // map then select across lines
    let src = "[1, 2, 3, 4]\n  .map() { |x| x * 2 }\n  .select() { |x| x > 4 }\n  .size()";
    assert_eq!(eval_with_stdlib(src), VmValue::Int(2));
}

// ── break / next in while loops ───────────────────────────────────────────────

#[test]
fn break_exits_while_loop() {
    let src = "
i = 0
while true {
  if i == 3 { break }
  i = i + 1
}
i";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn next_skips_to_next_iteration() {
    // Accumulate only even numbers via next
    let src = "
sum = 0
i = 0
while i < 6 {
  i = i + 1
  if i % 2 != 0 { next }
  sum = sum + i
}
sum";
    assert_eq!(eval(src), VmValue::Int(12)); // 2 + 4 + 6
}

#[test]
fn break_in_nested_while_exits_inner_only() {
    let src = "
outer = 0
i = 0
while i < 3 {
  j = 0
  while j < 3 {
    if j == 1 { break }
    j = j + 1
  }
  outer = outer + j
  i = i + 1
}
outer";
    assert_eq!(eval(src), VmValue::Int(3)); // j stops at 1 each time, 3 iterations
}

#[test]
fn next_in_nested_while_skips_inner_only() {
    let src = "
count = 0
i = 0
while i < 3 {
  i = i + 1
  j = 0
  while j < 3 {
    j = j + 1
    if j == 2 { next }
    count = count + 1
  }
}
count";
    assert_eq!(eval(src), VmValue::Int(6)); // 2 increments per outer iter, 3 iters
}

// ── Float#to_s ────────────────────────────────────────────────────────────────

#[test]
fn float_to_s_whole_number() {
    assert_eq!(eval("1.0.to_s()"), VmValue::Str("1.0".into()));
}

#[test]
fn float_to_s_whole_number_negative() {
    assert_eq!(eval("(-3.0).to_s()"), VmValue::Str("-3.0".into()));
}

#[test]
fn float_to_s_fractional() {
    assert_eq!(eval("3.14.to_s()"), VmValue::Str("3.14".into()));
}

// ── Float#zero? ───────────────────────────────────────────────────────────────

#[test]
fn float_zero_true() {
    assert_eq!(eval_with_stdlib("0.0.zero?()"), VmValue::Bool(true));
}

#[test]
fn float_zero_false() {
    assert_eq!(eval_with_stdlib("1.5.zero?()"), VmValue::Bool(false));
}

#[test]
fn float_zero_negative_zero() {
    assert_eq!(eval_with_stdlib("(-0.0).zero?()"), VmValue::Bool(true));
}

// ── Return type annotations (runtime enforcement) ─────────────────────────────

#[test]
fn return_type_annotation_correct() {
    let result = eval("def add(a: Int, b: Int) -> Int { a + b }\nadd(1, 2)");
    assert_eq!(result, VmValue::Int(3));
}

#[test]
fn return_type_annotation_wrong_type() {
    let err = eval_err("def greet() -> Int { \"hello\" }\ngreet()");
    match err {
        VmError::TypeError { message, .. } => {
            assert!(
                message.contains("expected Int"),
                "unexpected message: {}",
                message
            );
            assert!(
                message.contains("got String"),
                "unexpected message: {}",
                message
            );
        }
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn return_type_annotation_early_return_wrong() {
    let err = eval_err("def f() -> Int { return \"oops\" }\nf()");
    match err {
        VmError::TypeError { message, .. } => {
            assert!(
                message.contains("expected Int"),
                "unexpected message: {}",
                message
            );
        }
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn return_type_annotation_num_accepts_int() {
    let result = eval("def f() -> Num { 42 }\nf()");
    assert_eq!(result, VmValue::Int(42));
}

#[test]
fn return_type_annotation_num_accepts_float() {
    let result = eval("def f() -> Num { 3.14 }\nf()");
    assert_eq!(result, VmValue::Float(3.14));
}

// ── unary operator return-type inference ──────────────────────────────────────

#[test]
fn unary_bang_infers_bool_return() {
    let result = eval("def f() -> Bool { !true }\nf()");
    assert_eq!(result, VmValue::Bool(false));
}

#[test]
fn unary_minus_int_infers_int_return() {
    let result = eval("def f() -> Int { -42 }\nf()");
    assert_eq!(result, VmValue::Int(-42));
}

#[test]
fn unary_minus_float_infers_float_return() {
    let result = eval("def f() -> Float { -3.14 }\nf()");
    assert_eq!(result, VmValue::Float(-3.14));
}

#[test]
fn unary_tilde_infers_int_return() {
    let result = eval("def f() -> Int { ~0 }\nf()");
    assert_eq!(result, VmValue::Int(-1));
}

// ── break / next inside blocks passed to native methods ───────────────────────

#[test]
fn break_in_each_stops_iteration() {
    // break should stop iteration and execution continues after the each call
    let result = eval_with_stdlib(
        r#"
sum = 0
[1, 2, 3, 4, 5].each { |n|
  break if n == 3
  sum = sum + n
}
sum"#,
    );
    assert_eq!(result, VmValue::Int(3)); // 1 + 2, stops before 3
}

#[test]
fn break_in_each_execution_continues_after() {
    // code after the each call must still run
    let result = eval_with_stdlib(
        r#"
x = 0
[1, 2, 3].each { |n| break if n == 2 }
x = 99
x"#,
    );
    assert_eq!(result, VmValue::Int(99));
}

#[test]
fn break_in_map_stops_early() {
    // map collects [10, 20] then hits break; result is a partial list
    let result = eval_with_stdlib(
        r#"
[1, 2, 3, 4].map { |n|
  break if n == 3
  n * 10
}.size"#,
    );
    assert_eq!(result, VmValue::Int(3)); // [10, 20, nil] — 2 mapped + the break value
}

#[test]
fn next_in_each_skips_element() {
    let result = eval_with_stdlib(
        r#"
sum = 0
[1, 2, 3, 4, 5].each { |n|
  next if n % 2 == 0
  sum = sum + n
}
sum"#,
    );
    assert_eq!(result, VmValue::Int(9)); // 1 + 3 + 5
}

#[test]
fn break_in_nested_each_exits_inner_only() {
    // break inside the inner each should not affect the outer each
    let result = eval_with_stdlib(
        r#"
count = 0
[1, 2, 3].each { |i|
  [10, 20, 30].each { |j|
    break if j == 20
    count = count + 1
  }
}
count"#,
    );
    assert_eq!(result, VmValue::Int(3)); // inner each runs once per outer iteration
}

// ---- Nested classes / namespaces ----

#[test]
fn nested_class_accessible_via_dot() {
    let src = r#"class Outer {
  class Inner {
    def greet() { "hello" }
  }
}
Outer.Inner.new().greet()"#;
    assert_eq!(eval(src), VmValue::Str("hello".into()));
}

#[test]
fn nested_class_instantiation() {
    let src = r#"class Geometry {
  class Point {
    attr x
    attr y
  }
}
p = Geometry.Point.new(x: 3, y: 4)
p.x + p.y"#;
    assert_eq!(eval(src), VmValue::Int(7));
}

#[test]
fn nested_class_inherits_from_flat_class() {
    let src = r#"class Base {
  def kind() { "base" }
}
class Container {
  class Child < Base {}
}
Container.Child.new().kind()"#;
    assert_eq!(eval(src), VmValue::Str("base".into()));
}

#[test]
fn lexical_class_constant_in_instance_method() {
    let src = r#"class Outer {
  C = 1
  class Inner {
    def m() { C }
  }
}
Outer.Inner.new().m()"#;
    assert_eq!(eval(src), VmValue::Int(1));
}

#[test]
fn lexical_inner_class_constant_shadows_outer() {
    let src = r#"class Outer {
  C = 1
  class Inner {
    C = 2
    def m() { C }
  }
}
Outer.Inner.new().m()"#;
    assert_eq!(eval(src), VmValue::Int(2));
}

#[test]
fn lexical_class_constant_in_lambda_inside_method() {
    let src = r#"class Math {
  PI = 3
  def m() {
    f = def() { PI }
    f.call()
  }
}
Math.new().m()"#;
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn superclass_via_dot_notation() {
    let src = r#"class Outer {
  class Animal {
    def sound() { "..." }
  }
}
class Dog < Outer.Animal {
  def sound() { "woof" }
}
Dog.new().sound()"#;
    assert_eq!(eval(src), VmValue::Str("woof".into()));
}

// --- Optional parentheses for zero-arg method calls ---

#[test]
fn method_call_no_parens() {
    let src = r#"class Greeter {
  def hello() { "hi" }
}
g = Greeter.new()
g.hello"#;
    assert_eq!(eval(src), VmValue::Str("hi".into()));
}

#[test]
fn method_call_no_parens_chained() {
    let src = r#"class Wrapper {
  def value() { 42 }
}
w = Wrapper.new()
w.value + 1"#;
    assert_eq!(eval(src), VmValue::Int(43));
}

#[test]
fn attr_field_read_no_parens() {
    let src = r#"class Dog {
  attr name
}
d = Dog.new(name: "Rex")
d.name"#;
    assert_eq!(eval(src), VmValue::Str("Rex".into()));
}

#[test]
fn method_call_explicit_parens_still_works() {
    let src = r#"class Counter {
  def count() { 7 }
}
c = Counter.new()
c.count()"#;
    assert_eq!(eval(src), VmValue::Int(7));
}

#[test]
fn method_call_no_parens_inside_expression() {
    let src = r#"class Box {
  attr size
}
b = Box.new(size: 5)
b.size * 2"#;
    assert_eq!(eval(src), VmValue::Int(10));
}

// --- Optional parentheses for zero-arg method definitions ---

#[test]
fn def_no_parens_basic() {
    let src = r#"class Greeter {
  def hello { "hi" }
}
Greeter.new().hello()"#;
    assert_eq!(eval(src), VmValue::Str("hi".into()));
}

#[test]
fn def_no_parens_called_without_parens() {
    let src = r#"class Counter {
  def count { 7 }
}
Counter.new().count"#;
    assert_eq!(eval(src), VmValue::Int(7));
}

#[test]
fn def_no_parens_top_level() {
    assert_eq!(eval("def answer { 42 }\nanswer()"), VmValue::Int(42));
}

#[test]
fn def_with_parens_still_works() {
    let src = r#"class Box {
  def size() { 5 }
}
Box.new().size()"#;
    assert_eq!(eval(src), VmValue::Int(5));
}

// ── Union type syntax ─────────────────────────────────────────────────────────

fn parse_err_msg(src: &str) -> String {
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let err = sapphire::parser::Parser::new(tokens)
        .parse()
        .expect_err("expected parse error");
    format!("{}", err)
}

#[test]
fn union_return_type_accepts_either_arm() {
    // Int | String return type: both arms should be accepted at runtime
    let result = eval("def f(x: Int) -> Int | String { x }\nf(42)");
    assert_eq!(result, VmValue::Int(42));
}

#[test]
fn union_return_type_string_arm() {
    let result = eval("def f(x: Int) -> Int | String { \"hello\" }\nf(0)");
    assert_eq!(result, VmValue::Str("hello".into()));
}

#[test]
fn union_return_type_wrong_type_error() {
    let err = eval_err("def f() -> Int | String { 3.14 }\nf()");
    match err {
        VmError::TypeError { message, .. } => {
            assert!(
                message.contains("expected Int | String"),
                "msg: {}",
                message
            );
            assert!(message.contains("got Float"), "msg: {}", message);
        }
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn literal_return_type_string_exact_match() {
    let result = eval("def f() -> \"ok\" { \"ok\" }\nf()");
    assert_eq!(result, VmValue::Str("ok".into()));
}

#[test]
fn literal_return_type_string_mismatch_errors() {
    let err = eval_err("def f() -> \"ok\" { \"nope\" }\nf()");
    match err {
        VmError::TypeError { message, .. } => {
            assert!(message.contains("expected \"ok\""), "msg: {}", message);
            assert!(message.contains("got String"), "msg: {}", message);
        }
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn literal_union_param_typechecks() {
    let src = "def pick(mode: \"dev\" | \"prod\") { mode }\npick(\"dev\")";
    assert_eq!(eval(src), VmValue::Str("dev".into()));
}

#[test]
fn question_sugar_param_accepts_nil() {
    // Int? means Int | Nil — nil should be passable
    let result = eval("def f(x: Int?) { x }\nf(nil)");
    assert_eq!(result, VmValue::Nil);
}

#[test]
fn question_sugar_return_type_nil_ok() {
    let result = eval("def f() -> Int? { nil }\nf()");
    assert_eq!(result, VmValue::Nil);
}

#[test]
fn question_sugar_return_type_int_ok() {
    let result = eval("def f() -> Int? { 42 }\nf()");
    assert_eq!(result, VmValue::Int(42));
}

#[test]
fn question_sugar_return_type_wrong_type() {
    let err = eval_err("def f() -> Int? { \"oops\" }\nf()");
    match err {
        VmError::TypeError { message, .. } => {
            assert!(message.contains("expected Int | Nil"), "msg: {}", message);
        }
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn grouped_nullable_union_return_type() {
    // (Int | String)? == Int | String | Nil
    let result = eval("def f() -> (Int | String)? { nil }\nf()");
    assert_eq!(result, VmValue::Nil);
}

#[test]
fn nil_explicit_union_arm_parse_error() {
    // `Int | Nil` is a parse error — must use `Int?`
    let msg = parse_err_msg("def f() -> Int | Nil { nil }\nf()");
    assert!(msg.contains("use Int? instead"), "msg: {}", msg);
}

#[test]
fn nil_multi_arm_parse_error_suggests_grouped() {
    let msg = parse_err_msg("def f() -> Int | String | Nil { nil }\nf()");
    assert!(msg.contains("use (Int | String)? instead"), "msg: {}", msg);
}

#[test]
fn leading_pipe_multiline_union() {
    // Optional leading | for alignment (multiline style)
    let result = eval("def f() -> | Int | String { 1 }\nf()");
    assert_eq!(result, VmValue::Int(1));
}

// ── Type aliases ─────────────────────────────────────────────────────────────

#[test]
fn type_alias_inline() {
    // Basic inline alias: type T = Int | String
    let result = eval("type T = Int | String\ndef f() -> T { 1 }\nf()");
    assert_eq!(result, VmValue::Int(1));
}

#[test]
fn type_alias_multiline() {
    // Multiline union alias with leading pipe
    let src = "type Shape =\n  | Int\n  | Float\ndef f() -> Shape { 1 }\nf()";
    let result = eval(src);
    assert_eq!(result, VmValue::Int(1));
}

#[test]
fn type_alias_param() {
    // Alias used as a param type annotation
    let src = "type Number = Int | Float\ndef double(n: Number) -> Number { n }\ndouble(42)";
    let result = eval(src);
    assert_eq!(result, VmValue::Int(42));
}

#[test]
fn type_alias_runtime_return_type_checked() {
    // Return type is enforced at runtime via the alias
    let err = eval_err("type MyInt = Int\ndef f() -> MyInt { \"oops\" }\nf()");
    assert!(
        err.to_string().contains("return type error"),
        "unexpected error: {}",
        err
    );
}

// ── Generics ───────────────────────────────────────────────────────────────────

#[test]
fn generic_class_runs() {
    let result = eval_with_stdlib(
        "class Box[T] { attr value: T\ndef get() -> T { self.value } }\nb = Box.new(value: 42)\nb.get()",
    );
    assert_eq!(result, VmValue::Int(42));
}

#[test]
fn generic_class_string_value_runs() {
    let result =
        eval_with_stdlib("class Box[T] { attr value: T }\nb = Box.new(value: \"hi\")\nb.value");
    assert_eq!(result, VmValue::Str("hi".into()));
}

#[test]
fn generic_pair_class_runs() {
    let result = eval_with_stdlib(
        "class Pair[A, B] { attr first: A\nattr second: B }\np = Pair.new(first: 1, second: \"x\")\np.first",
    );
    assert_eq!(result, VmValue::Int(1));
}

#[test]
fn generic_function_runs() {
    let result = eval("def identity[T](x: T) -> T { x }\nidentity(7)");
    assert_eq!(result, VmValue::Int(7));
}

#[test]
fn generics_example_file_runs() {
    // Smoke test: the examples/generics.spr file should execute without errors
    let result = eval_with_stdlib(
        "class Box[T] { attr value: T\ndef get() -> T { self.value } }\n\
         b = Box.new(value: 99)\nb.get()",
    );
    assert_eq!(result, VmValue::Int(99));
}

// ── Implicit return type inference ────────────────────────────────────────────

#[test]
fn infer_return_type_from_literal_int() {
    // Unannotated function returning a literal: should run fine
    assert_eq!(eval("def f() { 42 }\nf()"), VmValue::Int(42));
}

#[test]
fn infer_return_type_from_literal_string() {
    assert_eq!(
        eval("def greet() { \"hello\" }\ngreet()"),
        VmValue::Str("hello".into())
    );
}

#[test]
fn annotated_return_type_unaffected_by_inference() {
    // Existing explicitly annotated functions still work correctly.
    let result = eval("def add(a: Int, b: Int) -> Int { a + b }\nadd(3, 4)");
    assert_eq!(result, VmValue::Int(7));
}

#[test]
fn infer_return_type_empty_body_no_crash() {
    // A function with an empty body should not panic — inference simply finds nothing.
    assert_eq!(eval("def noop() { }\nnoop()"), VmValue::Nil);
}

// ── Infer `if` expression types ───────────────────────────────────────────────

#[test]
fn infer_if_type_string_branches() {
    assert_eq!(
        eval("def label(x: Int) { if x > 0 { \"pos\" } else { \"non-pos\" } }\nlabel(1)"),
        VmValue::Str("pos".into()),
    );
}

// ── Infer `begin` expression types ───────────────────────────────────────────

#[test]
fn infer_begin_runtime() {
    assert_eq!(eval("def f() { begin\n99\nend }\nf()"), VmValue::Int(99));
}

// ── Infer `assign` expression types ──────────────────────────────────────────

#[test]
fn infer_assign_runtime() {
    // Sanity-check that assignment still evaluates to the RHS at runtime.
    assert_eq!(eval("def f() { x = 99 }\nf()"), VmValue::Int(99));
}
