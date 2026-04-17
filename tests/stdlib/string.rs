use super::{VmValue, eval};

#[test]
fn size() {
    assert_eq!(eval(r#""hello".size()"#), VmValue::Int(5));
    assert_eq!(eval(r#""".size()"#), VmValue::Int(0));
}

#[test]
fn upcase_downcase() {
    assert_eq!(eval(r#""hello".upcase()"#), VmValue::Str("HELLO".into()));
    assert_eq!(eval(r#""HELLO".downcase()"#), VmValue::Str("hello".into()));
}

#[test]
fn reverse() {
    assert_eq!(eval(r#""abc".reverse()"#), VmValue::Str("cba".into()));
    assert_eq!(eval(r#""".reverse()"#), VmValue::Str("".into()));
}

#[test]
fn strip() {
    assert_eq!(eval(r#""  hi  ".strip()"#), VmValue::Str("hi".into()));
    assert_eq!(eval(r#""hello".strip()"#), VmValue::Str("hello".into()));
}

#[test]
fn to_i() {
    assert_eq!(eval(r#""42".to_i()"#), VmValue::Int(42));
}

#[test]
fn to_f() {
    assert_eq!(eval(r#""3.14".to_f()"#), VmValue::Float(3.14));
}

#[test]
fn empty() {
    assert_eq!(eval(r#""".empty?()"#), VmValue::Bool(true));
    assert_eq!(eval(r#""x".empty?()"#), VmValue::Bool(false));
}

#[test]
fn include() {
    assert_eq!(eval(r#""hi".include?("i")"#), VmValue::Bool(true));
    assert_eq!(eval(r#""hi".include?("x")"#), VmValue::Bool(false));
}

#[test]
fn starts_with() {
    assert_eq!(eval(r#""hi".starts_with?("h")"#), VmValue::Bool(true));
    assert_eq!(eval(r#""hi".starts_with?("i")"#), VmValue::Bool(false));
}

#[test]
fn ends_with() {
    assert_eq!(eval(r#""hi".ends_with?("i")"#), VmValue::Bool(true));
    assert_eq!(eval(r#""hi".ends_with?("h")"#), VmValue::Bool(false));
}

#[test]
fn split() {
    let src = r#""a,b,c".split(",")"#;
    assert_eq!(eval(&format!("{}.size()", src)), VmValue::Int(3));
}

#[test]
fn chars() {
    assert_eq!(eval(r#""hi".chars().size()"#), VmValue::Int(2));
    assert_eq!(eval(r#""hi".chars()[0]"#), VmValue::Str("h".into()));
    assert_eq!(eval(r#""hi".chars()[1]"#), VmValue::Str("i".into()));
}
