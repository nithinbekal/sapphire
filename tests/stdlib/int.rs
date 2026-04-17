use super::{VmValue, eval};

#[test]
fn to_s() {
    assert_eq!(eval("42.to_s()"), VmValue::Str("42".into()));
}

#[test]
fn to_f() {
    assert_eq!(eval("42.to_f()"), VmValue::Float(42.0));
    assert_eq!(eval("3.to_f()"), VmValue::Float(3.0));
}

#[test]
fn abs() {
    assert_eq!(eval("n = -5\nn.abs()"), VmValue::Int(5));
    assert_eq!(eval("5.abs()"), VmValue::Int(5));
}

#[test]
fn even_odd() {
    assert_eq!(eval("4.even?()"), VmValue::Bool(true));
    assert_eq!(eval("3.even?()"), VmValue::Bool(false));
    assert_eq!(eval("3.odd?()"), VmValue::Bool(true));
    assert_eq!(eval("4.odd?()"), VmValue::Bool(false));
}

#[test]
fn zero() {
    assert_eq!(eval("0.zero?()"), VmValue::Bool(true));
    assert_eq!(eval("1.zero?()"), VmValue::Bool(false));
}

#[test]
fn max_min() {
    assert_eq!(eval("5.max(10)"), VmValue::Int(10));
    assert_eq!(eval("10.max(5)"), VmValue::Int(10));
    assert_eq!(eval("10.min(5)"), VmValue::Int(5));
    assert_eq!(eval("5.min(10)"), VmValue::Int(5));
}

#[test]
fn times() {
    let src = "sum = 0\n3.times() { |i| sum = sum + i }\nsum";
    assert_eq!(eval(src), VmValue::Int(3));
}
