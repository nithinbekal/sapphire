use sapphire::environment::Environment;
use sapphire::error::SapphireError;
use sapphire::interpreter::{execute, global_env};
use sapphire::lexer::Lexer;
use sapphire::parser::Parser;
use sapphire::value::{EnvRef, Value};

fn run(source: &str) -> Value {
    let tokens = Lexer::new(source).scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    execute(stmts.remove(0), global_env()).unwrap().unwrap()
}

fn run_env(source: &str, env: EnvRef) -> Value {
    let tokens = Lexer::new(source).scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    execute(stmts.remove(0), env).unwrap().unwrap()
}

fn exec_env(source: &str, env: EnvRef) {
    let tokens = Lexer::new(source).scan_tokens();
    let stmts = Parser::new(tokens).parse().unwrap();
    for stmt in stmts {
        execute(stmt, env.clone()).unwrap();
    }
}

fn run_all(source: &str) -> Value {
    let env = global_env();
    let tokens = Lexer::new(source).scan_tokens();
    let stmts = Parser::new(tokens).parse().unwrap();
    let mut result = Value::Nil;
    for stmt in stmts {
        if let Ok(Some(v)) = execute(stmt, env.clone()) {
            result = v;
        }
    }
    result
}

fn run_err(source: &str) -> SapphireError {
    let env = global_env();
    let tokens = Lexer::new(source).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    for stmt in stmts {
        if let Err(e) = execute(stmt, env.clone()) {
            return e;
        }
    }
    panic!("expected an error but none was raised");
}

#[test]
fn test_literal() { assert_eq!(run("42"), Value::Int(42)); }

#[test]
fn test_addition() { assert_eq!(run("1+2"), Value::Int(3)); }

#[test]
fn test_precedence() { assert_eq!(run("1+2*3"), Value::Int(7)); }

#[test]
fn test_grouping() { assert_eq!(run("(1+2)*3"), Value::Int(9)); }

#[test]
fn test_subtraction() { assert_eq!(run("10-3-2"), Value::Int(5)); }

#[test]
fn test_division() { assert_eq!(run("10/2"), Value::Int(5)); }

#[test]
fn test_division_by_zero() {
    let tokens = Lexer::new("1/0").scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    assert!(execute(stmts.remove(0), Environment::new()).is_err());
}

#[test]
fn test_modulo() {
    assert_eq!(run("10 % 3"), Value::Int(1));
    assert_eq!(run("9 % 3"), Value::Int(0));
}

#[test]
fn test_assign_and_read() {
    let env = Environment::new();
    assert_eq!(run_env("x = 10", env.clone()), Value::Int(10));
    assert_eq!(run_env("x", env.clone()), Value::Int(10));
}

#[test]
fn test_undefined_variable() {
    let tokens = Lexer::new("y").scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    assert!(execute(stmts.remove(0), Environment::new()).is_err());
}

#[test]
fn test_bool_literals() {
    assert_eq!(run("true"), Value::Bool(true));
    assert_eq!(run("false"), Value::Bool(false));
}

#[test]
fn test_equality() {
    assert_eq!(run("1 == 1"), Value::Bool(true));
    assert_eq!(run("1 == 2"), Value::Bool(false));
}

#[test]
fn test_comparison() {
    assert_eq!(run("1 < 2"), Value::Bool(true));
    assert_eq!(run("2 > 1"), Value::Bool(true));
    assert_eq!(run("1 <= 1"), Value::Bool(true));
    assert_eq!(run("1 >= 2"), Value::Bool(false));
}

#[test]
fn test_bang() {
    assert_eq!(run("!true"), Value::Bool(false));
    assert_eq!(run("!false"), Value::Bool(true));
}

#[test]
fn test_negate() {
    assert_eq!(run("-5"), Value::Int(-5));
    assert_eq!(run("-(1+2)"), Value::Int(-3));
}

#[test]
fn test_string_literal() {
    assert_eq!(run(r#""hello""#), Value::Str("hello".into()));
}

#[test]
fn test_string_concat() {
    assert_eq!(run(r#""hello" + " world""#), Value::Str("hello world".into()));
}

#[test]
fn test_string_equality() {
    assert_eq!(run(r#""a" == "a""#), Value::Bool(true));
    assert_eq!(run(r#""a" == "b""#), Value::Bool(false));
}

#[test]
fn test_while() {
    let env = Environment::new();
    exec_env("x = 0; while x < 3 { x = x + 1 }", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(3)));
}

#[test]
fn test_if_then() {
    let env = Environment::new();
    exec_env("x = 0; if true { x = 1 }", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(1)));
}

#[test]
fn test_if_else() {
    let env = Environment::new();
    exec_env("if false { x = 1 } else { x = 2 }", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(2)));
}

#[test]
fn test_if_condition() {
    let env = Environment::new();
    exec_env("x = 5; if x > 3 { x = 99 }", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(99)));
}

#[test]
fn test_if_expression_rhs() {
    let env = Environment::new();
    exec_env("x = if true { 1 } else { 42 }", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(1)));
    exec_env("x = if false { 1 } else { 42 }", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(42)));
}

#[test]
fn test_function_def_and_call() {
    let env = global_env();
    exec_env("def add(a, b) { a + b }", env.clone());
    assert_eq!(run_env("add(1, 2)", env), Value::Int(3));
}

#[test]
fn test_function_no_args() {
    let env = global_env();
    exec_env("def answer() { 42 }", env.clone());
    assert_eq!(run_env("answer()", env), Value::Int(42));
}

#[test]
fn test_function_closure() {
    let env = global_env();
    exec_env("x = 10; def get_x() { x }", env.clone());
    assert_eq!(run_env("get_x()", env), Value::Int(10));
}

#[test]
fn test_early_return() {
    let env = global_env();
    exec_env("def abs(n) { if n < 0 { return -n }; n }", env.clone());
    assert_eq!(run_env("abs(-5)", env.clone()), Value::Int(5));
    assert_eq!(run_env("abs(3)", env.clone()), Value::Int(3));
}

#[test]
fn test_class_instantiation() {
    let env = global_env();
    exec_env("class Point { attr x; attr y }", env.clone());
    exec_env("p = Point.new(x: 3, y: 2)", env.clone());
    assert_eq!(run_env("p.x", env.clone()), Value::Int(3));
    assert_eq!(run_env("p.y", env.clone()), Value::Int(2));
}

#[test]
fn test_instance_method() {
    let env = global_env();
    exec_env("class Point { attr x; attr y; def sum() { self.x + self.y } }", env.clone());
    exec_env("p = Point.new(x: 3, y: 2)", env.clone());
    assert_eq!(run_env("p.sum()", env.clone()), Value::Int(5));
}

#[test]
fn test_method_with_arg() {
    let env = global_env();
    exec_env("class Point { attr x; attr y; def translate(dx) { self.x + dx } }", env.clone());
    exec_env("p = Point.new(x: 3, y: 2)", env.clone());
    assert_eq!(run_env("p.translate(10)", env.clone()), Value::Int(13));
}

#[test]
fn test_string_length() {
    assert_eq!(run(r#""hello".length"#), Value::Int(5));
    assert_eq!(run(r#""".empty?"#), Value::Bool(true));
    assert_eq!(run(r#""hi".empty?"#), Value::Bool(false));
}

#[test]
fn test_string_case() {
    assert_eq!(run(r#""hello".upcase"#), Value::Str("HELLO".into()));
    assert_eq!(run(r#""HELLO".downcase"#), Value::Str("hello".into()));
}

#[test]
fn test_string_strip() {
    assert_eq!(run(r#""  hi  ".strip"#), Value::Str("hi".into()));
}

#[test]
fn test_string_include() {
    assert_eq!(run(r#""hello".include?("ell")"#), Value::Bool(true));
    assert_eq!(run(r#""hello".include?("xyz")"#), Value::Bool(false));
}

#[test]
fn test_string_starts_ends_with() {
    assert_eq!(run(r#""hello".starts_with?("hel")"#), Value::Bool(true));
    assert_eq!(run(r#""hello".ends_with?("llo")"#), Value::Bool(true));
    assert_eq!(run(r#""hello".starts_with?("xyz")"#), Value::Bool(false));
}

#[test]
fn test_string_split() {
    let result = run(r#""a,b,c".split(",")"#);
    if let Value::List(parts) = result {
        let parts = parts.borrow();
        assert_eq!(parts[0], Value::Str("a".into()));
        assert_eq!(parts[1], Value::Str("b".into()));
        assert_eq!(parts[2], Value::Str("c".into()));
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_to_s() {
    assert_eq!(run("42.to_s"), Value::Str("42".into()));
    assert_eq!(run("true.to_s"), Value::Str("true".into()));
    assert_eq!(run("nil.to_s()"), Value::Str("nil".into()));
}

#[test]
fn test_to_i() {
    assert_eq!(run(r#""42".to_i"#), Value::Int(42));
    assert_eq!(run("42.to_i"), Value::Int(42));
}

#[test]
fn test_safe_navigation_nil() {
    let env = Environment::new();
    exec_env("x = nil", env.clone());
    assert_eq!(run_env("x&.nil?", env.clone()), Value::Nil);
}

#[test]
fn test_safe_navigation_non_nil() {
    let env = global_env();
    exec_env("class Point { attr x }; p = Point.new(x: 3)", env.clone());
    assert_eq!(run_env("p&.x", env.clone()), Value::Int(3));
}

#[test]
fn test_nil_check() {
    assert_eq!(run("nil.nil?()"), Value::Bool(true));
    assert_eq!(run("42.nil?"), Value::Bool(false));
    assert_eq!(run("\"hello\".nil?"), Value::Bool(false));
    assert_eq!(run("false.nil?"), Value::Bool(false));
}

#[test]
fn test_each() {
    let env = Environment::new();
    exec_env("sum = 0; [1, 2, 3].each { |x| sum = sum + x }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(6)));
}

#[test]
fn test_it_each() {
    let env = Environment::new();
    exec_env("sum = 0; [1, 2, 3].each { sum = sum + it }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(6)));
}

#[test]
fn test_it_map() {
    let env = global_env();
    exec_env("result = [1, 2, 3].map { it * 2 }", env.clone());
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(2));
    assert_eq!(run_env("result[1]", env.clone()), Value::Int(4));
    assert_eq!(run_env("result[2]", env.clone()), Value::Int(6));
}

#[test]
fn test_it_not_set_when_params_explicit() {
    let env = Environment::new();
    exec_env("x = 0; [1, 2, 3].each { |n| x = n }", env.clone());
    assert_eq!(env.borrow().get("it"), None);
}

#[test]
fn test_map() {
    let env = global_env();
    exec_env("result = [1, 2, 3].map { |x| x * 2 }", env.clone());
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(2));
    assert_eq!(run_env("result[2]", env.clone()), Value::Int(6));
}

#[test]
fn test_select() {
    let env = global_env();
    exec_env("result = [1, 2, 3, 4].select { |x| x > 2 }", env.clone());
    assert_eq!(run_env("result.length", env.clone()), Value::Int(2));
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(3));
}

#[test]
fn test_reduce_with_initial() {
    assert_eq!(run("[1, 2, 3, 4, 5].reduce(0) { |acc, n| acc + n }"), Value::Int(15));
}

#[test]
fn test_reduce_without_initial() {
    assert_eq!(run("[1, 2, 3, 4, 5].reduce { |acc, n| acc * n }"), Value::Int(120));
}

#[test]
fn test_string_interp() {
    let env = Environment::new();
    exec_env("name = \"world\"", env.clone());
    assert_eq!(run_env(r#""hello #{name}""#, env.clone()), Value::Str("hello world".into()));
}

#[test]
fn test_string_interp_expr() {
    let env = Environment::new();
    exec_env("x = 3", env.clone());
    assert_eq!(run_env(r#""result: #{x * 2}""#, env.clone()), Value::Str("result: 6".into()));
}

#[test]
fn test_string_interp_int() {
    let env = Environment::new();
    exec_env("n = 42", env.clone());
    assert_eq!(run_env(r#""n is #{n}""#, env.clone()), Value::Str("n is 42".into()));
}

#[test]
fn test_list_literal() {
    let env = Environment::new();
    exec_env("a = [1, 2, 3]", env.clone());
    assert_eq!(run_env("a[0]", env.clone()), Value::Int(1));
    assert_eq!(run_env("a[2]", env.clone()), Value::Int(3));
}

#[test]
fn test_list_index_set() {
    let env = Environment::new();
    exec_env("a = [1, 2, 3]", env.clone());
    exec_env("a[0] = 99", env.clone());
    assert_eq!(run_env("a[0]", env.clone()), Value::Int(99));
}

#[test]
fn test_list_length() {
    let env = Environment::new();
    exec_env("a = [1, 2, 3]", env.clone());
    assert_eq!(run_env("a.length", env.clone()), Value::Int(3));
}

#[test]
fn test_list_push() {
    let env = Environment::new();
    exec_env("a = [1, 2]", env.clone());
    exec_env("a.push(3)", env.clone());
    assert_eq!(run_env("a.length", env.clone()), Value::Int(3));
    assert_eq!(run_env("a[2]", env.clone()), Value::Int(3));
}

#[test]
fn test_list_pop() {
    let env = Environment::new();
    exec_env("a = [1, 2, 3]", env.clone());
    assert_eq!(run_env("a.pop()", env.clone()), Value::Int(3));
    assert_eq!(run_env("a.length", env.clone()), Value::Int(2));
}

#[test]
fn test_and() {
    assert_eq!(run("true && true"), Value::Bool(true));
    assert_eq!(run("true && false"), Value::Bool(false));
    assert_eq!(run("false && true"), Value::Bool(false));
}

#[test]
fn test_or() {
    assert_eq!(run("true || false"), Value::Bool(true));
    assert_eq!(run("false || false"), Value::Bool(false));
    assert_eq!(run("false || true"), Value::Bool(true));
}

#[test]
fn test_inheritance_fields() {
    let env = global_env();
    exec_env("class Animal { attr name }", env.clone());
    exec_env("class Dog < Animal { attr breed }", env.clone());
    exec_env("d = Dog.new(name: \"Rex\", breed: \"Lab\")", env.clone());
    assert_eq!(run_env("d.name", env.clone()), Value::Str("Rex".into()));
    assert_eq!(run_env("d.breed", env.clone()), Value::Str("Lab".into()));
}

#[test]
fn test_inheritance_method() {
    let env = global_env();
    exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
    exec_env("class Dog < Animal {}", env.clone());
    exec_env("d = Dog.new(name: \"Rex\")", env.clone());
    assert_eq!(run_env("d.speak()", env.clone()), Value::Str("...".into()));
}

#[test]
fn test_inheritance_override() {
    let env = global_env();
    exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
    exec_env("class Dog < Animal { def speak() { \"woof\" } }", env.clone());
    exec_env("d = Dog.new(name: \"Rex\")", env.clone());
    assert_eq!(run_env("d.speak()", env.clone()), Value::Str("woof".into()));
}

#[test]
fn test_field_mutation() {
    let env = global_env();
    exec_env("class Counter { attr n; def inc() { self.n = self.n + 1 } }", env.clone());
    exec_env("c = Counter.new(n: 0)", env.clone());
    exec_env("c.inc()", env.clone());
    assert_eq!(run_env("c.n", env.clone()), Value::Int(1));
}

#[test]
fn test_class_default_field() {
    let env = global_env();
    exec_env(r#"class Point { attr x; attr y; attr label = "origin" }"#, env.clone());
    exec_env("p = Point.new(x: 1, y: 2)", env.clone());
    assert_eq!(run_env("p.label", env.clone()), Value::Str("origin".into()));
}

#[test]
fn test_while_break() {
    let env = Environment::new();
    exec_env("x = 0; while true { x = x + 1; break if x == 3 }", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(3)));
}

#[test]
fn test_while_next() {
    let env = Environment::new();
    exec_env("x = 0; sum = 0; while x < 5 { x = x + 1; next if x == 3; sum = sum + x }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(12)));
}

#[test]
fn test_each_next() {
    let env = Environment::new();
    exec_env("sum = 0; [1, 2, 3, 4, 5].each { |x| next if x == 3; sum = sum + x }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(12)));
}

#[test]
fn test_each_break() {
    let env = Environment::new();
    exec_env("sum = 0; [1, 2, 3, 4, 5].each { |x| break if x == 4; sum = sum + x }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(6)));
}

#[test]
fn test_super() {
    let env = global_env();
    exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
    exec_env("class Dog < Animal { def speak() { super.speak() } }", env.clone());
    exec_env("d = Dog.new(name: \"Rex\")", env.clone());
    assert_eq!(run_env("d.speak()", env.clone()), Value::Str("...".into()));
}

#[test]
fn test_super_with_override() {
    let env = global_env();
    exec_env("class Animal { attr name; def describe() { self.name } }", env.clone());
    exec_env("class Dog < Animal { attr breed; def describe() { super.describe() + \" (\" + self.breed + \")\" } }", env.clone());
    exec_env("d = Dog.new(name: \"Rex\", breed: \"Lab\")", env.clone());
    assert_eq!(run_env("d.describe()", env.clone()), Value::Str("Rex (Lab)".into()));
}

#[test]
fn test_map_next() {
    let env = global_env();
    exec_env("result = [1, 2, 3].map { |x| next 0 if x == 2; x * 2 }", env.clone());
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(2));
    assert_eq!(run_env("result[1]", env.clone()), Value::Int(0));
    assert_eq!(run_env("result[2]", env.clone()), Value::Int(6));
}

#[test]
fn test_yield_basic() {
    let env = global_env();
    exec_env("def call_block() { yield(42) }", env.clone());
    assert_eq!(run_env("call_block() { |x| x * 2 }", env.clone()), Value::Int(84));
}

#[test]
fn test_yield_multiple_args() {
    let env = global_env();
    exec_env("def call_block(a, b) { yield(a, b) }", env.clone());
    assert_eq!(run_env("call_block(3, 4) { |x, y| x + y }", env.clone()), Value::Int(7));
}

#[test]
fn test_yield_in_loop() {
    let env = global_env();
    exec_env("def my_each(list) { len = list.length; i = 0; while i < len { yield(list[i]); i = i + 1 } }", env.clone());
    exec_env("sum = 0; my_each([1, 2, 3]) { |x| sum = sum + x }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(6)));
}

#[test]
fn test_yield_in_method() {
    let env = global_env();
    exec_env("class Wrapper { attr items; def each() { len = self.items.length; i = 0; while i < len { yield(self.items[i]); i = i + 1 } } }", env.clone());
    exec_env("w = Wrapper.new(items: [10, 20, 30])", env.clone());
    exec_env("sum = 0; w.each() { |x| sum = sum + x }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(60)));
}

#[test]
fn test_constant_assignment() {
    let env = global_env();
    exec_env("MAX = 100", env.clone());
    assert_eq!(run_env("MAX", env.clone()), Value::Int(100));
}

#[test]
fn test_constant_reassignment_is_error() {
    let env = global_env();
    exec_env("PI = 3.14", env.clone());
    let tokens = Lexer::new("PI = 3").scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    assert!(execute(stmts.remove(0), env).is_err());
}

#[test]
fn test_mixed_case_is_not_a_constant() {
    let env = global_env();
    exec_env("Pi = 3.14", env.clone());
    exec_env("Pi = 3", env.clone());
    assert_eq!(run_env("Pi", env.clone()), Value::Int(3));
}

#[test]
fn test_constant_is_readable_in_methods() {
    let env = global_env();
    exec_env("MAX = 10", env.clone());
    exec_env("def cap(n) { if n > MAX { MAX } else { n } }", env.clone());
    assert_eq!(run_env("cap(5)", env.clone()), Value::Int(5));
    assert_eq!(run_env("cap(20)", env.clone()), Value::Int(10));
}

#[test]
fn test_user_class_cannot_be_redefined() {
    let env = global_env();
    exec_env("class Dog { attr name }", env.clone());
    let tokens = Lexer::new("class Dog { attr breed }").scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    assert!(execute(stmts.remove(0), env).is_err());
}

#[test]
fn test_reserved_class_cannot_be_redefined() {
    let env = global_env();
    let tokens = Lexer::new("class List { attr x }").scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    assert!(execute(stmts.remove(0), env).is_err());
}

#[test]
fn test_map_delete() {
    let env = Environment::new();
    exec_env(r#"m = { name: "Alice", age: 30 }"#, env.clone());
    exec_env(r#"m.delete("name")"#, env.clone());
    assert_eq!(run_env("m.length", env.clone()), Value::Int(1));
    assert_eq!(run_env(r#"m.has_key?("name")"#, env.clone()), Value::Bool(false));
}

#[test]
fn test_map_merge() {
    let env = Environment::new();
    exec_env(r#"a = { x: 1 }; b = { y: 2 }"#, env.clone());
    exec_env("c = a.merge(b)", env.clone());
    assert_eq!(run_env("c.length", env.clone()), Value::Int(2));
    assert_eq!(run_env(r#"c["x"]"#, env.clone()), Value::Int(1));
    assert_eq!(run_env(r#"c["y"]"#, env.clone()), Value::Int(2));
}

#[test]
fn test_map_select() {
    let env = global_env();
    exec_env(r#"m = { a: 1, b: 2, c: 3 }; result = m.select { |k, v| v > 1 }"#, env.clone());
    assert_eq!(run_env("result.length", env.clone()), Value::Int(2));
    assert_eq!(run_env(r#"result.has_key?("a")"#, env.clone()), Value::Bool(false));
    assert_eq!(run_env(r#"result.has_key?("b")"#, env.clone()), Value::Bool(true));
}

#[test]
fn test_map_any() {
    let env = global_env();
    exec_env(r#"m = { a: 1, b: 2 }"#, env.clone());
    assert_eq!(run_env("m.any? { |k, v| v > 1 }", env.clone()), Value::Bool(true));
    assert_eq!(run_env("m.any? { |k, v| v > 9 }", env.clone()), Value::Bool(false));
}

#[test]
fn test_map_all() {
    let env = global_env();
    exec_env(r#"m = { a: 1, b: 2 }"#, env.clone());
    assert_eq!(run_env("m.all? { |k, v| v > 0 }", env.clone()), Value::Bool(true));
    assert_eq!(run_env("m.all? { |k, v| v > 1 }", env.clone()), Value::Bool(false));
}

#[test]
fn test_map_none() {
    let env = global_env();
    exec_env(r#"m = { a: 1, b: 2 }"#, env.clone());
    assert_eq!(run_env("m.none? { |k, v| v > 9 }", env.clone()), Value::Bool(true));
    assert_eq!(run_env("m.none? { |k, v| v > 1 }", env.clone()), Value::Bool(false));
}

#[test]
fn test_reserved_map_cannot_be_redefined() {
    let env = global_env();
    let tokens = Lexer::new("class Map { attr x }").scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    assert!(execute(stmts.remove(0), env).is_err());
}

#[test]
fn test_implicit_self_field_read() {
    let env = global_env();
    exec_env("class Point { attr x; attr y; def sum() { x + y } }", env.clone());
    exec_env("p = Point.new(x: 3, y: 4)", env.clone());
    assert_eq!(run_env("p.sum()", env.clone()), Value::Int(7));
}

#[test]
fn test_implicit_self_method_call() {
    let env = global_env();
    exec_env("class Counter { attr count; def increment() { self.count = count + 1 }; def value() { count } }", env.clone());
    exec_env("c = Counter.new(count: 0)", env.clone());
    exec_env("c.increment()", env.clone());
    exec_env("c.increment()", env.clone());
    assert_eq!(run_env("c.value()", env.clone()), Value::Int(2));
}

#[test]
fn test_implicit_self_local_shadows_field() {
    let env = global_env();
    exec_env("class Box { attr x; def doubled() { x = 99; x } }", env.clone());
    exec_env("b = Box.new(x: 10)", env.clone());
    assert_eq!(run_env("b.doubled()", env.clone()), Value::Int(99));
    assert_eq!(run_env("b.x", env.clone()), Value::Int(10));
}

#[test]
fn test_elsif_second_branch() {
    let env = Environment::new();
    exec_env("x = 5; result = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }", env.clone());
    assert_eq!(env.borrow().get("result"), Some(Value::Int(2)));
}

#[test]
fn test_elsif_else_branch() {
    let env = Environment::new();
    exec_env("x = 1; result = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }", env.clone());
    assert_eq!(env.borrow().get("result"), Some(Value::Int(3)));
}

#[test]
fn test_elsif_first_branch() {
    let env = Environment::new();
    exec_env("x = 20; result = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }", env.clone());
    assert_eq!(env.borrow().get("result"), Some(Value::Int(1)));
}

#[test]
fn test_elsif_chain() {
    let env = Environment::new();
    exec_env("x = 5; result = 0\nif x == 1 { result = 1 } elsif x == 2 { result = 2 } elsif x == 5 { result = 5 } else { result = 99 }", env.clone());
    assert_eq!(env.borrow().get("result"), Some(Value::Int(5)));
}

#[test]
fn test_defp_callable_from_within_class() {
    let env = global_env();
    exec_env("class Foo { attr x; defp secret() { x + 1 }; def pub() { secret() } }", env.clone());
    exec_env("f = Foo.new(x: 10)", env.clone());
    assert_eq!(run_env("f.pub()", env.clone()), Value::Int(11));
}

#[test]
fn test_defp_blocked_from_outside() {
    let env = global_env();
    exec_env("class Foo { defp secret() { 42 } }", env.clone());
    exec_env("f = Foo.new()", env.clone());
    let tokens = Lexer::new("f.secret()").scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    assert!(execute(stmts.remove(0), env).is_err());
}

#[test]
fn test_defp_inherited_callable_from_subclass() {
    let env = global_env();
    exec_env("class A { defp helper() { 99 }; def run() { helper() } }", env.clone());
    exec_env("class B < A { }", env.clone());
    exec_env("b = B.new()", env.clone());
    assert_eq!(run_env("b.run()", env.clone()), Value::Int(99));
}

#[test]
fn test_downto() {
    let env = global_env();
    exec_env("result = []; 3.downto(1) { |i| result.push(i) }", env.clone());
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(3));
    assert_eq!(run_env("result[2]", env.clone()), Value::Int(1));
}

#[test]
fn test_string_chars() {
    let env = global_env();
    exec_env(r#"result = "hi".chars"#, env.clone());
    assert_eq!(run_env("result.length", env.clone()), Value::Int(2));
    assert_eq!(run_env("result[0]", env.clone()), Value::Str("h".into()));
    assert_eq!(run_env("result[1]", env.clone()), Value::Str("i".into()));
}

#[test]
fn test_multi_assign_basic() {
    let env = Environment::new();
    exec_env("a, b = 1, 2", env.clone());
    assert_eq!(env.borrow().get("a"), Some(Value::Int(1)));
    assert_eq!(env.borrow().get("b"), Some(Value::Int(2)));
}

#[test]
fn test_multi_assign_swap() {
    let env = Environment::new();
    exec_env("a = 1; b = 2", env.clone());
    exec_env("a, b = b, a", env.clone());
    assert_eq!(env.borrow().get("a"), Some(Value::Int(2)));
    assert_eq!(env.borrow().get("b"), Some(Value::Int(1)));
}

#[test]
fn test_multi_assign_three() {
    let env = Environment::new();
    exec_env("x, y, z = 10, 20, 30", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(10)));
    assert_eq!(env.borrow().get("y"), Some(Value::Int(20)));
    assert_eq!(env.borrow().get("z"), Some(Value::Int(30)));
}

#[test]
fn test_while_condition_method_call_no_block_greed() {
    let env = global_env();
    exec_env("list = [1, 2, 3]; i = 0; sum = 0; while i < list.length { sum = sum + list[i]; i = i + 1 }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(6)));
}

#[test]
fn test_raise_unhandled() {
    let tokens = Lexer::new(r#"raise "oops""#).scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    let result = execute(stmts.remove(0), Environment::new());
    assert!(matches!(result, Err(SapphireError::Raised(Value::Str(_)))));
}

#[test]
fn test_begin_rescue_catches_raise() {
    let env = Environment::new();
    exec_env(r#"x = 0; begin; raise "err"; rescue e; x = 1; end"#, env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(1)));
}

#[test]
fn test_begin_rescue_binds_message() {
    let env = Environment::new();
    exec_env(r#"begin; raise "boom"; rescue e; end"#, env.clone());
    assert_eq!(env.borrow().get("e"), Some(Value::Str("boom".into())));
}

#[test]
fn test_begin_rescue_catches_runtime_error() {
    let env = Environment::new();
    exec_env("x = 0; begin; x = 1 / 0; rescue e; x = 99; end", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(99)));
}

#[test]
fn test_begin_else_runs_when_no_error() {
    let env = Environment::new();
    exec_env("x = 0\nbegin\n  x = 1\nrescue e\n  x = 99\nelse\n  x = 2\nend", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(2)));
}

#[test]
fn test_begin_else_skipped_on_error() {
    let env = Environment::new();
    exec_env("x = 0\nbegin\n  raise \"err\"\nrescue e\n  x = 99\nelse\n  x = 2\nend", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(99)));
}

#[test]
fn test_begin_no_error_skips_rescue() {
    let env = Environment::new();
    exec_env("x = 0; begin; x = 42; rescue e; x = 1; end", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(42)));
}

#[test]
fn test_begin_expr_value_body() {
    let env = Environment::new();
    exec_env("x = begin 7 end", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(7)));
}

#[test]
fn test_begin_expr_else_overrides_body_value() {
    let env = Environment::new();
    // `else` must follow `rescue` (Ruby); else value wins on success.
    exec_env("x = begin 1 rescue e 0 else 2 end", env.clone());
    assert_eq!(env.borrow().get("x"), Some(Value::Int(2)));
}

#[test]
fn test_inline_rescue_in_function() {
    let env = global_env();
    exec_env("def risky(x) { raise \"bad\" if x < 0\n x * 2\nrescue e\n 0 }", env.clone());
    assert_eq!(run_env("risky(5)", env.clone()), Value::Int(10));
    assert_eq!(run_env("risky(-1)", env.clone()), Value::Int(0));
}

#[test]
fn test_inline_rescue_binds_error() {
    let env = global_env();
    exec_env("def boom() { raise \"oops\"\n 1\nrescue e\n e }", env.clone());
    assert_eq!(run_env("boom()", env.clone()), Value::Str("oops".into()));
}

#[test]
fn test_inline_rescue_in_method() {
    let env = global_env();
    exec_env("class Safe { def try_div(x) { 10 / x\nrescue e\n -1 } }", env.clone());
    exec_env("s = Safe.new()", env.clone());
    assert_eq!(run_env("s.try_div(2)", env.clone()), Value::Int(5));
    assert_eq!(run_env("s.try_div(0)", env.clone()), Value::Int(-1));
}

#[test]
fn test_raise_instance() {
    let env = global_env();
    exec_env("class Err { attr msg }; begin; raise Err.new(msg: \"bad\"); rescue e; end", env.clone());
    if let Some(Value::Instance { class_name, .. }) = env.borrow().get("e") {
        assert_eq!(class_name, "Err");
    } else {
        panic!("expected instance");
    }
}

#[test]
fn test_object_class_registered() {
    let env = global_env();
    assert!(matches!(env.borrow().get("Object"), Some(Value::Class { .. })));
}

#[test]
fn test_object_cannot_be_redefined() {
    let env = global_env();
    let tokens = Lexer::new("class Object {}").scan_tokens();
    let mut stmts = Parser::new(tokens).parse().unwrap();
    assert!(execute(stmts.remove(0), env).is_err());
}

#[test]
fn test_implicit_object_superclass() {
    let env = global_env();
    exec_env("class Animal { attr name }", env.clone());
    if let Some(Value::Class { superclass, .. }) = env.borrow().get("Animal") {
        assert_eq!(superclass, Some("Object".to_string()));
    } else {
        panic!("Animal not found");
    }
}

#[test]
fn test_is_a_direct_class() {
    let env = global_env();
    exec_env("class Dog { attr name }; d = Dog.new(name: \"Rex\")", env.clone());
    assert_eq!(run_env("d.is_a?(Dog)", env.clone()), Value::Bool(true));
}

#[test]
fn test_is_a_superclass() {
    let env = global_env();
    exec_env("class Animal { attr name }; class Dog < Animal { attr breed }; d = Dog.new(name: \"Rex\", breed: \"Lab\")", env.clone());
    assert_eq!(run_env("d.is_a?(Animal)", env.clone()), Value::Bool(true));
}

#[test]
fn test_is_a_object() {
    let env = global_env();
    exec_env("class Foo {}; f = Foo.new()", env.clone());
    assert_eq!(run_env("f.is_a?(Object)", env.clone()), Value::Bool(true));
}

#[test]
fn test_is_a_unrelated_class() {
    let env = global_env();
    exec_env("class Cat {}; class Dog {}; d = Dog.new()", env.clone());
    assert_eq!(run_env("d.is_a?(Cat)", env.clone()), Value::Bool(false));
}

#[test]
fn test_is_a_deep_chain() {
    let env = global_env();
    exec_env("class A {}; class B < A {}; class C < B {}; c = C.new()", env.clone());
    assert_eq!(run_env("c.is_a?(C)", env.clone()), Value::Bool(true));
    assert_eq!(run_env("c.is_a?(B)", env.clone()), Value::Bool(true));
    assert_eq!(run_env("c.is_a?(A)", env.clone()), Value::Bool(true));
    assert_eq!(run_env("c.is_a?(Object)", env.clone()), Value::Bool(true));
}

#[test]
fn test_num_is_a_superclass_of_int_and_float() {
    let env = global_env();
    assert_eq!(run_env("1.is_a?(Num)", env.clone()), Value::Bool(true));
    assert_eq!(run_env("1.0.is_a?(Num)", env.clone()), Value::Bool(true));
    assert_eq!(run_env("\"hi\".is_a?(Num)", env.clone()), Value::Bool(false));
}

#[test]
fn test_num_methods_on_int_and_float() {
    let env = global_env();
    assert_eq!(run_env("0.zero?()", env.clone()), Value::Bool(true));
    assert_eq!(run_env("0.0.zero?()", env.clone()), Value::Bool(true));
    assert_eq!(run_env("3.positive?()", env.clone()), Value::Bool(true));
    assert_eq!(run_env("(-1.0).negative?()", env.clone()), Value::Bool(true));
    assert_eq!(run_env("(-5).abs()", env.clone()), Value::Int(5));
    assert_eq!(run_env("(-2.5).abs()", env.clone()), Value::Float(2.5));
    assert_eq!(run_env("10.clamp(1, 5)", env.clone()), Value::Int(5));
    assert_eq!(run_env("0.clamp(1, 5)", env.clone()), Value::Int(1));
}

#[test]
fn test_num_type_annotation_accepts_int_and_float() {
    let env = global_env();
    exec_env("def double(x: Num) { x + x }", env.clone());
    assert_eq!(run_env("double(3)", env.clone()), Value::Int(6));
    assert_eq!(run_env("double(1.5)", env.clone()), Value::Float(3.0));
}

#[test]
fn test_super_still_works_with_implicit_object() {
    let env = global_env();
    exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
    exec_env("class Dog < Animal { def speak() { super.speak() } }", env.clone());
    exec_env("d = Dog.new(name: \"Rex\")", env.clone());
    assert_eq!(run_env("d.speak()", env.clone()), Value::Str("...".into()));
}

#[test]
fn test_range_literal() {
    assert_eq!(run("1..10"), Value::Range { from: 1, to: 10 });
}

#[test]
fn test_range_each() {
    let env = global_env();
    exec_env("sum = 0; (1..5).each { |i| sum = sum + i }", env.clone());
    assert_eq!(env.borrow().get("sum"), Some(Value::Int(15)));
}

#[test]
fn test_range_include() {
    let env = global_env();
    assert_eq!(run_env("(1..10).include?(5)", env.clone()), Value::Bool(true));
    assert_eq!(run_env("(1..10).include?(11)", env.clone()), Value::Bool(false));
    assert_eq!(run_env("(1..10).include?(1)", env.clone()), Value::Bool(true));
    assert_eq!(run_env("(1..10).include?(10)", env.clone()), Value::Bool(true));
}

#[test]
fn test_range_to_s() {
    assert_eq!(run("(1..5).to_s"), Value::Str("1..5".into()));
}

#[test]
fn test_float_literal() {
    assert_eq!(run("3.14"), Value::Float(3.14));
    assert_eq!(run("1.0"), Value::Float(1.0));
}

#[test]
fn test_float_arithmetic() {
    assert_eq!(run("1.5 + 2.5"), Value::Float(4.0));
    assert_eq!(run("3.0 - 1.5"), Value::Float(1.5));
    assert_eq!(run("2.0 * 3.0"), Value::Float(6.0));
    assert_eq!(run("7.0 / 2.0"), Value::Float(3.5));
}

#[test]
fn test_float_mixed_arithmetic() {
    assert_eq!(run("1 + 0.5"), Value::Float(1.5));
    assert_eq!(run("0.5 + 1"), Value::Float(1.5));
    assert_eq!(run("3 * 1.5"), Value::Float(4.5));
    assert_eq!(run("7 / 2.0"), Value::Float(3.5));
}

#[test]
fn test_int_division_stays_int() {
    assert_eq!(run("7 / 2"), Value::Int(3));
}

#[test]
fn test_float_comparison() {
    assert_eq!(run("1.5 < 2.0"), Value::Bool(true));
    assert_eq!(run("2.0 > 1.5"), Value::Bool(true));
    assert_eq!(run("1.0 == 1.0"), Value::Bool(true));
    assert_eq!(run("1.0 == 1"), Value::Bool(true));
    assert_eq!(run("1 == 1.0"), Value::Bool(true));
}

#[test]
fn test_float_negation() {
    assert_eq!(run("-3.14"), Value::Float(-3.14));
}

#[test]
fn test_float_to_i() {
    assert_eq!(run("3.9.to_i"), Value::Int(3));
    assert_eq!(run("-3.9.to_i"), Value::Int(-3));
}

#[test]
fn test_int_to_f() {
    assert_eq!(run("3.to_f"), Value::Float(3.0));
}

#[test]
fn test_float_to_s() {
    assert_eq!(run("3.14.to_s"), Value::Str("3.14".into()));
    assert_eq!(run("1.0.to_s"), Value::Str("1.0".into()));
}

#[test]
fn test_string_escape_newline() {
    assert_eq!(run(r#""\n""#), Value::Str("\n".into()));
}

#[test]
fn test_string_escape_tab() {
    assert_eq!(run(r#""\t""#), Value::Str("\t".into()));
}

#[test]
fn test_string_escape_backslash() {
    assert_eq!(run(r#""\\""#), Value::Str("\\".into()));
}

#[test]
fn test_string_escape_quote() {
    assert_eq!(run(r#""\"""#), Value::Str("\"".into()));
}

#[test]
fn test_string_escape_in_interpolation() {
    assert_eq!(run(r#""a\nb""#), Value::Str("a\nb".into()));
}

#[test]
fn test_list_first_no_arg() {
    assert_eq!(run("[1, 2, 3].first()"), Value::Int(1));
}

#[test]
fn test_list_first_n() {
    let env = global_env();
    exec_env("result = [1, 2, 3, 4, 5].first(3)", env.clone());
    assert_eq!(run_env("result.length", env.clone()), Value::Int(3));
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(1));
    assert_eq!(run_env("result[2]", env.clone()), Value::Int(3));
}

#[test]
fn test_list_last_no_arg() {
    assert_eq!(run("[1, 2, 3].last()"), Value::Int(3));
}

#[test]
fn test_list_last_n() {
    let env = global_env();
    exec_env("result = [1, 2, 3, 4, 5].last(2)", env.clone());
    assert_eq!(run_env("result.length", env.clone()), Value::Int(2));
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(4));
    assert_eq!(run_env("result[1]", env.clone()), Value::Int(5));
}

#[test]
fn test_list_count_no_block() {
    assert_eq!(run("[1, 2, 3].count()"), Value::Int(3));
}

#[test]
fn test_list_count_with_block() {
    let env = global_env();
    assert_eq!(run_env("[1, 2, 3, 4].count { |x| x > 2 }", env), Value::Int(2));
}

#[test]
fn test_list_sort() {
    let env = global_env();
    exec_env("result = [3, 1, 4, 1, 5, 9, 2].sort()", env.clone());
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(1));
    assert_eq!(run_env("result[1]", env.clone()), Value::Int(1));
    assert_eq!(run_env("result[6]", env.clone()), Value::Int(9));
}

#[test]
fn test_list_sort_strings() {
    let env = global_env();
    exec_env(r#"result = ["banana", "apple", "cherry"].sort()"#, env.clone());
    assert_eq!(run_env("result[0]", env.clone()), Value::Str("apple".into()));
    assert_eq!(run_env("result[2]", env.clone()), Value::Str("cherry".into()));
}

#[test]
fn test_list_sort_by() {
    let env = global_env();
    exec_env(r#"words = ["banana", "fig", "apple"]; result = words.sort_by { |w| w.length }"#, env.clone());
    assert_eq!(run_env("result[0]", env.clone()), Value::Str("fig".into()));
    assert_eq!(run_env("result[2]", env.clone()), Value::Str("banana".into()));
}

#[test]
fn test_list_flatten() {
    let env = global_env();
    exec_env("result = [[1, 2], [3, [4, 5]]].flatten()", env.clone());
    assert_eq!(run_env("result.length", env.clone()), Value::Int(5));
    assert_eq!(run_env("result[3]", env.clone()), Value::Int(4));
}

#[test]
fn test_list_uniq() {
    let env = global_env();
    exec_env("result = [1, 2, 2, 3, 1].uniq()", env.clone());
    assert_eq!(run_env("result.length", env.clone()), Value::Int(3));
    assert_eq!(run_env("result[0]", env.clone()), Value::Int(1));
    assert_eq!(run_env("result[2]", env.clone()), Value::Int(3));
}

#[test]
fn test_list_each_with_index() {
    let env = global_env();
    exec_env("pairs = []; [\"a\", \"b\", \"c\"].each_with_index { |item, i| pairs.push(i) }", env.clone());
    assert_eq!(run_env("pairs[0]", env.clone()), Value::Int(0));
    assert_eq!(run_env("pairs[1]", env.clone()), Value::Int(1));
    assert_eq!(run_env("pairs[2]", env.clone()), Value::Int(2));
}

#[test]
fn test_list_include() {
    let env = global_env();
    assert_eq!(run_env("[1, 2, 3].include?(2)", env.clone()), Value::Bool(true));
    assert_eq!(run_env("[1, 2, 3].include?(9)", env.clone()), Value::Bool(false));
}

#[test]
fn test_list_zip() {
    let env = global_env();
    exec_env("result = [1, 2, 3].zip([4, 5, 6])", env.clone());
    assert_eq!(run_env("result.length", env.clone()), Value::Int(3));
    if let Value::List(pair) = run_env("result[0]", env.clone()) {
        assert_eq!(pair.borrow()[0], Value::Int(1));
        assert_eq!(pair.borrow()[1], Value::Int(4));
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_typed_param_accepts_correct_type() {
    assert_eq!(run_all("def add(a: Int, b: Int) { a + b }; add(1, 2)"), Value::Int(3));
}

#[test]
fn test_typed_param_rejects_wrong_type() {
    let err = run_err(r#"def add(a: Int, b: Int) { a + b }; add("x", 2)"#);
    assert!(matches!(err, SapphireError::TypeError { .. }));
    if let SapphireError::TypeError { message } = err {
        assert!(
            message.contains("'a'") && message.contains("Int") && message.contains("String"),
            "unexpected message: {}",
            message
        );
    }
}

#[test]
fn test_typed_return_accepts_correct_type() {
    assert_eq!(run_all("def f() -> Int { 42 }; f()"), Value::Int(42));
}

#[test]
fn test_typed_return_rejects_wrong_type() {
    let err = run_err(r#"def f() -> Int { "oops" }; f()"#);
    assert!(matches!(err, SapphireError::TypeError { .. }));
}

#[test]
fn test_attr_type_enforced_on_constructor() {
    let err = run_err(r#"class P { attr x: Int }; P.new(x: "bad")"#);
    assert!(matches!(err, SapphireError::TypeError { .. }));
    if let SapphireError::TypeError { message } = err {
        assert!(
            message.contains("'x'") && message.contains("Int") && message.contains("String"),
            "unexpected message: {}",
            message
        );
    }
}

#[test]
fn test_attr_type_accepted_on_constructor() {
    let env = global_env();
    exec_env("class P { attr x: Int }", env.clone());
    exec_env("p = P.new(x: 42)", env.clone());
    assert_eq!(run_env("p.x", env), Value::Int(42));
}

#[test]
fn test_attr_type_enforced_on_set() {
    let err = run_err(r#"class P { attr x: Int }; p = P.new(x: 1); p.x = "bad""#);
    assert!(matches!(err, SapphireError::TypeError { .. }));
}

#[test]
fn test_attr_type_accepted_on_set() {
    let env = global_env();
    exec_env("class P { attr x: Int }", env.clone());
    exec_env("p = P.new(x: 1)", env.clone());
    exec_env("p.x = 99", env.clone());
    assert_eq!(run_env("p.x", env), Value::Int(99));
}

#[test]
fn test_unannotated_code_unchanged() {
    assert_eq!(run_all(r#"def greet(name) { name }; greet("Alice")"#), Value::Str("Alice".into()));
}

#[test]
fn test_method_typed_param_rejects_wrong_type() {
    let err = run_err(r#"class Calc { def double(n: Int) { n * 2 } }; Calc.new().double("x")"#);
    assert!(matches!(err, SapphireError::TypeError { .. }));
}

#[test]
fn test_method_return_type_rejects_wrong_type() {
    let err = run_err(r#"class Calc { def num() -> Int { "nope" } }; Calc.new().num()"#);
    assert!(matches!(err, SapphireError::TypeError { .. }));
}
