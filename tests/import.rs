use sapphire::compiler::compile;
use sapphire::lexer::Lexer;
use sapphire::parser::Parser;
use sapphire::vm::{Vm, VmValue};
use std::path::PathBuf;

/// Run source code with the given directory as the "current file" location,
/// so that relative imports resolve correctly.
fn run_with_dir(src: &str, dir: PathBuf) -> VmValue {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    let mut vm = Vm::new(func, dir);
    vm.load_stdlib().expect("stdlib");
    vm.run().expect("vm error").expect("empty stack")
}

fn imports_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("imports")
        .canonicalize()
        .unwrap()
}

#[test]
fn import_functions_from_file() {
    let dir = imports_dir();
    let result = run_with_dir(
        r#"import "./math_utils"
square(5)"#,
        dir,
    );
    assert_eq!(result, VmValue::Int(25));
}

#[test]
fn import_multiple_functions() {
    let dir = imports_dir();
    let result = run_with_dir(
        r#"import "./math_utils"
cube(3)"#,
        dir,
    );
    assert_eq!(result, VmValue::Int(27));
}

#[test]
fn import_class_and_instantiate() {
    let dir = imports_dir();
    let result = run_with_dir(
        r#"import "./point"
p = Point.new(x: 10, y: 20)
p.x"#,
        dir,
    );
    assert_eq!(result, VmValue::Int(10));
}

#[test]
fn import_class_method_call() {
    let dir = imports_dir();
    let result = run_with_dir(
        r#"import "./point"
p = Point.new(x: 3, y: 4)
p.to_s()"#,
        dir,
    );
    assert_eq!(result, VmValue::Str("(3, 4)".into()));
}

#[test]
fn import_from_subdirectory() {
    let dir = imports_dir();
    let result = run_with_dir(
        r#"import "./sub/greeting"
greet("world")"#,
        dir,
    );
    assert_eq!(result, VmValue::Str("hello world".into()));
}

#[test]
fn import_is_deduplicated() {
    // Importing the same file twice should not double-execute it.
    // We verify by checking that `square` is available and no error occurs.
    let dir = imports_dir();
    let result = run_with_dir(
        r#"import "./math_utils"
import "./math_utils"
square(4)"#,
        dir,
    );
    assert_eq!(result, VmValue::Int(16));
}

#[test]
fn import_nonexistent_file_errors() {
    let dir = imports_dir();
    let tokens = Lexer::new(r#"import "./no_such_file""#).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    let mut vm = Vm::new(func, dir);
    vm.load_stdlib().expect("stdlib");
    let err = vm.run().expect_err("expected error");
    let msg = format!("{}", err);
    assert!(
        msg.contains("not found") || msg.contains("file not found"),
        "unexpected: {}",
        msg
    );
}

#[test]
fn import_path_must_be_relative() {
    // The parser should reject non-relative paths at parse time.
    let tokens = Lexer::new(r#"import "point""#).scan_tokens();
    let err = Parser::new(tokens)
        .parse()
        .expect_err("expected parse error");
    let msg = format!("{}", err);
    assert!(msg.contains("relative"), "unexpected: {}", msg);
}
