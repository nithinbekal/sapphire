/// First type error message from `src`, or panics if the program is accepted.
fn typecheck_err_msg(src: &str) -> String {
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens)
        .parse()
        .expect("parse error");
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(!errors.is_empty(), "expected type errors for:\n{src}");
    errors[0].message.clone()
}

/// Asserts typecheck fails; the first error message must contain every `substring` (e.g. expected and got for mismatches).
macro_rules! assert_typecheck_error {
    ($src:expr, $($substring:expr),+ $(,)?) => {
        {
            let msg = typecheck_err_msg($src);
            $(
            assert!(
                msg.contains($substring),
                "expected first type error to contain:\n{}\n\nmessage:\n{}",
                $substring,
                msg
            );
            )*
        }
    };
}

fn typecheck_ok(src: &str) {
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens).parse().unwrap();
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(errors.is_empty(), "unexpected type errors: {:?}", errors);
}

fn check_types_ok(src: &str) -> sapphire::typechecker::CheckedTypes {
    use sapphire::typechecker::TypeChecker;
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens).parse().unwrap();
    let info = TypeChecker::check_info(&stmts);
    assert!(info.errors.is_empty(), "unexpected type errors: {:?}", info.errors);
    info.types
}

/// Asserts a top-level function's resolved return type; `$ty` is a bare name (`Int`, `String`, `MyClass`, …).
macro_rules! assert_function_returns {
    ($types:expr, $name:expr, $ty:ident) => {
        assert_eq!(
            $types.function_return_type($name),
            Some(Some(sapphire::ast::TypeExpr::Named(stringify!($ty).to_string())))
        );
    };
}

/// Asserts a class method's resolved return type; same `$ty` convention as [`assert_function_returns!`].
macro_rules! assert_method_returns {
    ($types:expr, $class:expr, $method:expr, $ty:ident) => {
        assert_eq!(
            $types.method_return_type($class, $method),
            Some(Some(sapphire::ast::TypeExpr::Named(stringify!($ty).to_string())))
        );
    };
}

#[test]
fn literal_union_param_rejects_wrong_literal() {
    assert_typecheck_error!(
        "def pick(mode: \"dev\" | \"prod\") { mode }\npick(\"test\")",
        "expected \"dev\" | \"prod\", got String"
    );
}

#[test]
fn union_duplicate_arm_type_error() {
    assert_typecheck_error!("def f() -> Int | Int { 1 }\nf()", "duplicate type 'Int' in union");
}

#[test]
fn union_duplicate_param_arm_type_error() {
    assert_typecheck_error!(
        "def f(x: String | String) { x }\nf(\"hi\")",
        "duplicate type 'String' in union"
    );
}

#[test]
fn type_alias_typechecker_resolves() {
    typecheck_ok("type Number = Int | Float\ndef f(n: Number) { n }\nf(1)");
}

#[test]
fn parameterized_type_annotation_no_errors() {
    let types = check_types_ok("def sum(items: List[Int]) -> Int { 0 }");
    assert_function_returns!(types, "sum", Int);
}

#[test]
fn generic_type_var_compatible_with_itself() {
    let types = check_types_ok(
        "class Box[T] { attr value: T\ndef get() -> T { self.value } }",
    );
    assert_method_returns!(types, "Box", "get", T);
}

#[test]
fn apply_same_type_args_compatible() {
    // Param and return use the same `List[Int]`; both arms structurally match.
    typecheck_ok("def f(x: List[Int]) { x }");
}

#[test]
fn bare_list_compatible_with_parameterized_list_gradual() {
    typecheck_ok("def process(items: List[Int]) { items }\nprocess([])");
}

#[test]
fn infer_return_type_propagates_to_annotated_caller() {
    let types = check_types_ok(
        "def double(n: Int) { n * 2 }\n\
         def wrapper() -> Int { double(3) }",
    );
    assert_function_returns!(types, "double", Int);
}

#[test]
fn inferred_top_level_function_return_type_exposed() {
    let types = check_types_ok(
        "def double(n: Int) { n * 2 }\n\
         def wrapper { double(3) }",
    );
    assert_function_returns!(types, "double", Int);
}

#[test]
fn inferred_class_method_return_type_exposed() {
    let types = check_types_ok(
        "class C {\n\
          def m(n: Int) { n * 2 }\n\
        }",
    );
    assert_method_returns!(types, "C", "m", Int);
}

#[test]
fn infer_return_type_catches_caller_mismatch() {
    assert_typecheck_error!(
        "def greet() { \"hello\" }\n\
         def main() -> Int { greet() }",
        "expected Int",
        "got String"
    );
}

#[test]
fn infer_return_type_class_method_propagates() {
    let types = check_types_ok(
        "class Counter {\n\
           def value() { 0 }\n\
           def doubled() -> Int { self.value() }\n\
         }",
    );
    assert_method_returns!(types, "Counter", "value", Int);
    assert_method_returns!(types, "Counter", "doubled", Int);
}

#[test]
fn infer_if_type_matching_branches() {
    let types = check_types_ok(
        "def clamp(x: Int) { if x > 0 { x } else { 0 } }\n\
         def caller() -> Int { clamp(5) }",
    );
    assert_function_returns!(types, "clamp", Int);
}

#[test]
fn infer_if_type_catches_caller_mismatch() {
    assert_typecheck_error!(
        "def sign(x: Int) { if x > 0 { 1 } else { 0 } }\n\
         def caller() -> String { sign(1) }",
        "expected String",
        "got Int"
    );
}

#[test]
fn infer_if_no_else_no_inference() {
    typecheck_ok("def maybe(x: Int) { if x > 0 { x } }\nmaybe(1)");
}

#[test]
fn infer_if_mismatched_branches_no_inference() {
    typecheck_ok(
        "def mixed(x: Int) { if x > 0 { 1 } else { \"neg\" } }\nmixed(1)",
    );
}

#[test]
fn infer_begin_type_no_rescue() {
    let types = check_types_ok(
        "def f() { begin\n42\nend }\n\
         def caller() -> Int { f() }",
    );
    assert_function_returns!(types, "f", Int);
}

#[test]
fn infer_begin_type_catches_caller_mismatch() {
    assert_typecheck_error!(
        "def f() { begin\n42\nend }\n\
         def caller() -> String { f() }",
        "expected String",
        "got Int"
    );
}

#[test]
fn infer_begin_type_with_rescue_no_inference() {
    typecheck_ok("def f() { begin\n42\nrescue e\n0\nend }\nf()");
}

#[test]
fn infer_assign_propagates_int() {
    let types = check_types_ok(
        "def f() { x = 42 }\n\
         def caller() -> Int { f() }",
    );
    assert_function_returns!(types, "f", Int);
}

#[test]
fn infer_assign_propagates_string() {
    let types = check_types_ok(
        "def f() { s = \"hello\" }\n\
         def caller() -> String { f() }",
    );
    assert_function_returns!(types, "f", String);
}

#[test]
fn infer_assign_catches_caller_mismatch() {
    assert_typecheck_error!(
        "def f() { x = 1 }\n\
         def caller() -> String { f() }",
        "expected String",
        "got Int"
    );
}

#[test]
fn infer_assign_chained_through_variable() {
    let types = check_types_ok(
        "def f(n: Int) { x = n }\n\
         def caller() -> Int { f(7) }",
    );
    assert_function_returns!(types, "f", Int);
}

#[test]
fn safe_get_call_infers_nil_union() {
    use sapphire::ast::TypeExpr;
    let types = check_types_ok(
        "class Foo {\n\
           def bar() -> Int { 42 }\n\
         }\n\
         def f(x: Foo) { x&.bar() }",
    );
    assert_eq!(
        types.function_return_type("f"),
        Some(Some(TypeExpr::Union(vec![
            TypeExpr::Named("Nil".into()),
            TypeExpr::Named("Int".into()),
        ])))
    );
}

#[test]
fn safe_get_call_rejects_when_int_expected() {
    assert_typecheck_error!(
        "class Foo {\n\
           def bar() -> Int { 42 }\n\
         }\n\
         def f(x: Foo) -> Int { x&.bar() }",
        "expected Int",
        "got Nil | Int"
    );
}

#[test]
fn safe_get_call_accepts_nullable_return() {
    typecheck_ok(
        "class Foo {\n\
           def bar() -> Int { 42 }\n\
         }\n\
         def f(x: Foo) -> Int? { x&.bar() }",
    );
}
