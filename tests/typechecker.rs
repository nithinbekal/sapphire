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
fn infer_while_returns_nil() {
    let types = check_types_ok(
        "def f() { while true { 42 } }\n\
         def caller() -> Nil { f() }",
    );
    assert_function_returns!(types, "f", Nil);
}

#[test]
fn infer_while_nil_catches_caller_mismatch() {
    assert_typecheck_error!(
        "def f() { while true { 42 } }\n\
         def caller() -> Int { f() }",
        "expected Int",
        "got Nil"
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

#[test]
fn infer_return_expr_int() {
    let types = check_types_ok(
        "def f() { return 42 }\n\
         def caller() -> Int { f() }",
    );
    assert_function_returns!(types, "f", Int);
}

#[test]
fn infer_return_expr_string() {
    let types = check_types_ok(
        "def f() { return \"hi\" }\n\
         def caller() -> String { f() }",
    );
    assert_function_returns!(types, "f", String);
}

#[test]
fn infer_return_expr_catches_caller_mismatch() {
    assert_typecheck_error!(
        "def f() { return 42 }\n\
         def caller() -> String { f() }",
        "expected String",
        "got Int"
    );
}

#[test]
fn infer_early_return_method() {
    let types = check_types_ok(
        "class C {\n\
           def f(x: Int) { return x }\n\
         }\n\
         def caller() -> Int { C.new().f(1) }",
    );
    assert_method_returns!(types, "C", "f", Int);
}

#[test]
fn infer_set_propagates_int() {
    let types = check_types_ok(
        "class P { attr x: Int\n def f() { self.x = 42 } }\n\
         def caller() -> Int { P.new().f() }",
    );
    assert_method_returns!(types, "P", "f", Int);
}

#[test]
fn infer_set_propagates_string() {
    let types = check_types_ok(
        "class P { attr label: String\n def f() { self.label = \"hi\" } }\n\
         def caller() -> String { P.new().f() }",
    );
    assert_method_returns!(types, "P", "f", String);
}

#[test]
fn infer_set_catches_caller_mismatch() {
    assert_typecheck_error!(
        "class P { attr x: Int\n def f() { self.x = 1 } }\n\
         def caller() -> String { P.new().f() }",
        "expected String",
        "got Int"
    );
}

#[test]
fn infer_set_chained_through_typed_param() {
    let types = check_types_ok(
        "class P { attr x: Int\n def f(n: Int) { self.x = n } }\n\
         def caller() -> Int { P.new().f(7) }",
    );
    assert_method_returns!(types, "P", "f", Int);
}

#[test]
fn infer_string_concat_plus() {
    let types = check_types_ok(
        "def greet(a: String, b: String) { a + b }\n\
         def caller() -> String { greet(\"hello\", \" world\") }",
    );
    assert_function_returns!(types, "greet", String);
}

#[test]
fn infer_string_concat_return_type_mismatch() {
    assert_typecheck_error!(
        "def f(a: String, b: String) { a + b }\n\
         def caller() -> Int { f(\"x\", \"y\") }",
        "expected Int",
        "got String"
    );
}

#[test]
fn string_plus_int_does_not_infer() {
    let types = check_types_ok(
        "def f(a: String, b: Int) { a + b }\n\
         def caller() -> String { \"ok\" }",
    );
    assert_eq!(types.function_return_type("f"), Some(None));
}

#[test]
fn infer_callee_defined_after_caller() {
    let types = check_types_ok(
        "def caller() { callee() }\n\
         def callee() { 42 }",
    );
    assert_function_returns!(types, "caller", Int);
    assert_function_returns!(types, "callee", Int);
}

#[test]
fn infer_chained_forward_references() {
    let types = check_types_ok(
        "def a() { b() }\n\
         def b() { c() }\n\
         def c() { 42 }",
    );
    assert_function_returns!(types, "a", Int);
    assert_function_returns!(types, "b", Int);
    assert_function_returns!(types, "c", Int);
}

#[test]
fn infer_method_calling_forward_declared_method_in_same_class() {
    let types = check_types_ok(
        "class C {\n\
           def caller() { self.callee() }\n\
           def callee() { 42 }\n\
         }",
    );
    assert_method_returns!(types, "C", "caller", Int);
    assert_method_returns!(types, "C", "callee", Int);
}

#[test]
fn infer_function_calling_method_on_later_declared_class() {
    let types = check_types_ok(
        "def caller() { B.new().helper() }\n\
         class B {\n\
           def helper() { 42 }\n\
         }",
    );
    assert_function_returns!(types, "caller", Int);
    assert_method_returns!(types, "B", "helper", Int);
}

#[test]
fn infer_list_literal_index() {
    let types = check_types_ok(
        "def f() { [1, 2, 3][0] }\n\
         def caller() -> Int { f() }",
    );
    assert_function_returns!(types, "f", Int);
}

#[test]
fn infer_list_literal_index_catches_mismatch() {
    assert_typecheck_error!(
        "def f() { [1, 2, 3][0] }\n\
         def caller() -> String { f() }",
        "expected String",
        "got Int"
    );
}

#[test]
fn infer_list_param_index() {
    let types = check_types_ok(
        "def first(items: List[Int]) { items[0] }\n\
         def caller() -> Int { first([]) }",
    );
    assert_function_returns!(types, "first", Int);
}

#[test]
fn infer_map_literal_index() {
    let types = check_types_ok(
        "def f() { {a: 1, b: 2}[\"a\"] }\n\
         def caller() -> Int { f() }",
    );
    assert_function_returns!(types, "f", Int);
}

#[test]
fn infer_mixed_list_literal_index_no_inference() {
    // Mixed element types → no element type inference, so return type stays unknown
    typecheck_ok("def f() { [1, \"two\", 3][0] }");
}

#[test]
fn infer_multiassign_return_type() {
    let types = check_types_ok(
        "def f() { a, b = 1, 2 }\n\
         def caller() -> Int { f() }",
    );
    assert_function_returns!(types, "f", Int);
}

#[test]
fn infer_multiassign_spread_list_literal() {
    typecheck_ok(
        "a, b = [1, 2]\n\
         def f(x: Int, y: Int) { }\n\
         f(a, b)",
    );
}

#[test]
fn infer_multiassign_spread_list_call() {
    typecheck_ok(
        "def g() -> List[Int] { [] }\n\
         a, b = g()\n\
         def f(x: Int, y: Int) { }\n\
         f(a, b)",
    );
}

// ── Mutual-recursion fixed-point inference ─────────────────────────────────────

#[test]
fn infer_mutual_recursion_with_base_case() {
    // f has a concrete base-case (Int); g delegates to f.
    // The lenient second pass should resolve both to Int.
    let types = check_types_ok(
        "def f(n: Int) { if n > 0 { g(n - 1) } else { 0 } }\n\
         def g(n: Int) { f(n) }",
    );
    assert_function_returns!(types, "f", Int);
    assert_function_returns!(types, "g", Int);
}

#[test]
fn infer_mutual_recursion_base_in_else() {
    // Base case lives in the else branch instead of then.
    let types = check_types_ok(
        "def f(n: Int) { if n == 0 { g(n) } else { 42 } }\n\
         def g(n: Int) { f(n - 1) }",
    );
    assert_function_returns!(types, "f", Int);
    assert_function_returns!(types, "g", Int);
}

#[test]
fn infer_mutual_recursion_three_functions() {
    // Three-way cycle: f → g → h → f, with a base case in f.
    let types = check_types_ok(
        "def f(n: Int) { if n == 0 { 0 } else { g(n - 1) } }\n\
         def g(n: Int) { h(n) }\n\
         def h(n: Int) { f(n) }",
    );
    assert_function_returns!(types, "f", Int);
    assert_function_returns!(types, "g", Int);
    assert_function_returns!(types, "h", Int);
}

#[test]
fn infer_mutual_recursion_pure_cycle_stays_none() {
    // Pure cycle with no base case — both should remain None.
    let types = check_types_ok(
        "def f() { g() }\n\
         def g() { f() }",
    );
    assert_eq!(types.function_return_type("f"), Some(None));
    assert_eq!(types.function_return_type("g"), Some(None));
}

#[test]
fn infer_mutual_recursion_catches_mismatch() {
    // After resolving mutual recursion, the inferred type should
    // trigger a mismatch when a caller expects the wrong type.
    assert_typecheck_error!(
        "def f(n: Int) { if n > 0 { g(n - 1) } else { 0 } }\n\
         def g(n: Int) { f(n) }\n\
         def caller() -> String { g(1) }",
        "expected String",
        "got Int"
    );
}

#[test]
fn infer_mutual_recursion_methods() {
    // Mutual recursion between two methods in the same class.
    let types = check_types_ok(
        "class C {\n\
           def f(n: Int) { if n == 0 { 0 } else { self.g(n - 1) } }\n\
           def g(n: Int) { self.f(n) }\n\
         }",
    );
    assert_method_returns!(types, "C", "f", Int);
    assert_method_returns!(types, "C", "g", Int);
}

#[test]
fn infer_self_recursive_with_base_case() {
    // Single self-recursive function — already worked before the
    // lenient pass but should still pass.
    let types = check_types_ok(
        "def fact(n: Int) { if n <= 1 { 1 } else { n * fact(n - 1) } }",
    );
    assert_function_returns!(types, "fact", Int);
}
