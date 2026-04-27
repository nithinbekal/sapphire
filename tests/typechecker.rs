fn typecheck_err_msg(src: &str) -> String {
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens)
        .parse()
        .expect("parse error");
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(!errors.is_empty(), "expected type errors");
    errors[0].message.clone()
}

fn typecheck_ok(src: &str) {
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens).parse().unwrap();
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(errors.is_empty(), "unexpected type errors: {:?}", errors);
}

#[test]
fn literal_union_param_rejects_wrong_literal() {
    let msg = typecheck_err_msg("def pick(mode: \"dev\" | \"prod\") { mode }\npick(\"test\")");
    assert!(
        msg.contains("expected \"dev\" | \"prod\", got String"),
        "msg: {}",
        msg
    );
}

#[test]
fn union_duplicate_arm_type_error() {
    let msg = typecheck_err_msg("def f() -> Int | Int { 1 }\nf()");
    assert!(msg.contains("duplicate type 'Int' in union"), "msg: {}", msg);
}

#[test]
fn union_duplicate_param_arm_type_error() {
    let msg = typecheck_err_msg("def f(x: String | String) { x }\nf(\"hi\")");
    assert!(msg.contains("duplicate type 'String' in union"), "msg: {}", msg);
}

#[test]
fn type_alias_typechecker_resolves() {
    let src = "type Number = Int | Float\ndef f(n: Number) { n }\nf(1)";
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens).parse().unwrap();
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(errors.is_empty(), "unexpected type errors: {:?}", errors);
}

#[test]
fn parameterized_type_annotation_no_errors() {
    let src = "def sum(items: List[Int]) -> Int { 0 }";
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens).parse().unwrap();
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(errors.is_empty(), "unexpected type errors: {:?}", errors);
}

#[test]
fn generic_type_var_compatible_with_itself() {
    let src = "class Box[T] { attr value: T\ndef get() -> T { self.value } }";
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens).parse().unwrap();
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(errors.is_empty(), "unexpected type errors: {:?}", errors);
}

#[test]
fn apply_same_type_args_compatible() {
    use sapphire::ast::TypeExpr;
    let list_int = TypeExpr::Apply("List".into(), vec![TypeExpr::Named("Int".into())]);
    let list_int2 = TypeExpr::Apply("List".into(), vec![TypeExpr::Named("Int".into())]);
    let src = "def f(x: List[Int]) { x }";
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens).parse().unwrap();
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    let _ = (list_int, list_int2);
}

#[test]
fn bare_list_compatible_with_parameterized_list_gradual() {
    let src = "def process(items: List[Int]) { items }\nprocess([])";
    let tokens = sapphire::lexer::Lexer::new(src).scan_tokens();
    let stmts = sapphire::parser::Parser::new(tokens).parse().unwrap();
    let errors = sapphire::typechecker::TypeChecker::check(&stmts);
    assert!(errors.is_empty(), "unexpected type errors: {:?}", errors);
}

#[test]
fn infer_return_type_propagates_to_annotated_caller() {
    typecheck_ok(
        "def double(n: Int) { n * 2 }\n\
         def wrapper() -> Int { double(3) }",
    );
}

#[test]
fn infer_return_type_catches_caller_mismatch() {
    let msg = typecheck_err_msg(
        "def greet() { \"hello\" }\n\
         def main() -> Int { greet() }",
    );
    assert!(msg.contains("expected Int"), "unexpected message: {}", msg);
    assert!(msg.contains("got String"), "unexpected message: {}", msg);
}

#[test]
fn infer_return_type_class_method_propagates() {
    typecheck_ok(
        "class Counter {\n\
           def value() { 0 }\n\
           def doubled() -> Int { self.value() }\n\
         }",
    );
}

#[test]
fn infer_if_type_matching_branches() {
    typecheck_ok(
        "def clamp(x: Int) { if x > 0 { x } else { 0 } }\n\
         def caller() -> Int { clamp(5) }",
    );
}

#[test]
fn infer_if_type_catches_caller_mismatch() {
    let msg = typecheck_err_msg(
        "def sign(x: Int) { if x > 0 { 1 } else { 0 } }\n\
         def caller() -> String { sign(1) }",
    );
    assert!(msg.contains("expected String"), "msg: {}", msg);
    assert!(msg.contains("got Int"), "msg: {}", msg);
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
    typecheck_ok(
        "def f() { begin\n42\nend }\n\
         def caller() -> Int { f() }",
    );
}

#[test]
fn infer_begin_type_catches_caller_mismatch() {
    let msg = typecheck_err_msg(
        "def f() { begin\n42\nend }\n\
         def caller() -> String { f() }",
    );
    assert!(msg.contains("expected String"), "msg: {}", msg);
    assert!(msg.contains("got Int"), "msg: {}", msg);
}

#[test]
fn infer_begin_type_with_rescue_no_inference() {
    typecheck_ok("def f() { begin\n42\nrescue e\n0\nend }\nf()");
}

#[test]
fn infer_assign_propagates_int() {
    typecheck_ok(
        "def f() { x = 42 }\n\
         def caller() -> Int { f() }",
    );
}

#[test]
fn infer_assign_propagates_string() {
    typecheck_ok(
        "def f() { s = \"hello\" }\n\
         def caller() -> String { f() }",
    );
}

#[test]
fn infer_assign_catches_caller_mismatch() {
    let msg = typecheck_err_msg(
        "def f() { x = 1 }\n\
         def caller() -> String { f() }",
    );
    assert!(msg.contains("expected String"), "msg: {}", msg);
    assert!(msg.contains("got Int"), "msg: {}", msg);
}

#[test]
fn infer_assign_chained_through_variable() {
    typecheck_ok(
        "def f(n: Int) { x = n }\n\
         def caller() -> Int { f(7) }",
    );
}
