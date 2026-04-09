use sapphire::compiler::compile;
use sapphire::lexer::Lexer;
use sapphire::parser::Parser;
use sapphire::vm::{Vm, VmError, VmValue};

fn eval(src: &str) -> VmValue {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    Vm::new(func).run().expect("vm error").expect("empty stack")
}

fn eval_with_stdlib(src: &str) -> VmValue {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    let mut vm = Vm::new(func);
    vm.load_stdlib().expect("stdlib");
    vm.run().expect("vm error").expect("empty stack")
}

fn eval_err(src: &str) -> VmError {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    Vm::new(func).run().expect_err("expected vm error")
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
    assert_eq!(eval("x = 0\nif false { x = 1 } else { x = 2 }\nx"), VmValue::Int(2));
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

#[test]
fn last_expr_is_implicit_return() {
    let tokens = Lexer::new("1 + 1\n2 + 2").scan_tokens();
    let stmts = Parser::new(tokens).parse().unwrap();
    let func = compile(&stmts).unwrap();
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
  def speak() { \"dog:\" + super.speak() }
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
  def add(x, y) { super.add(x, y) + 1 }
}
Child.new().add(2, 3)";
    assert_eq!(eval(src), VmValue::Int(6));
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
        eval(r#"m = {x: 1, y: 2}
m["x"]"#),
        VmValue::Int(1)
    );
}

#[test]
fn map_missing_key_is_nil() {
    assert_eq!(
        eval(r#"m = {a: 1}
m["z"]"#),
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
    assert_eq!(
        eval("def f() { print 42 }\nf()"),
        VmValue::Int(42)
    );
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
    assert_eq!(eval("n = -5\nn.abs()"), VmValue::Int(5));
    assert_eq!(eval("4.even?()"), VmValue::Bool(true));
    assert_eq!(eval("3.odd?()"), VmValue::Bool(true));
    assert_eq!(eval("0.zero?()"), VmValue::Bool(true));
}

#[test]
fn float_methods() {
    assert_eq!(eval("3.7.round()"), VmValue::Int(4));
    assert_eq!(eval("3.7.floor()"), VmValue::Int(3));
    assert_eq!(eval("3.2.ceil()"), VmValue::Int(4));
    assert_eq!(eval("3.5.to_i()"), VmValue::Int(3));
    assert_eq!(eval("n = -2.5\nn.abs()"), VmValue::Float(2.5));
}

#[test]
fn string_methods() {
    assert_eq!(eval(r#""hello".size()"#), VmValue::Int(5));
    assert_eq!(eval(r#""hello".upcase()"#), VmValue::Str("HELLO".into()));
    assert_eq!(eval(r#""HELLO".downcase()"#), VmValue::Str("hello".into()));
    assert_eq!(eval(r#""abc".reverse()"#), VmValue::Str("cba".into()));
    assert_eq!(eval(r#""  hi  ".strip()"#), VmValue::Str("hi".into()));
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
    assert_eq!(eval("a = [1,2]\na.push(3)\na.size()"), VmValue::Int(3));
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
    assert_eq!(eval("false.nil?()"), VmValue::Bool(false));
    assert_eq!(eval("nil.to_s()"), VmValue::Str("".into()));
    assert_eq!(eval("true.to_s()"), VmValue::Str("true".into()));
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
    assert_eq!(eval("n = 10; f = def(x) { x + n }; f.call(3)"), VmValue::Int(13));
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
