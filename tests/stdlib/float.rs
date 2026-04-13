use super::{eval, VmValue};

#[test]
fn round() {
    assert_eq!(eval("3.7.round()"), VmValue::Int(4));
    assert_eq!(eval("3.4.round()"), VmValue::Int(3));
}

#[test]
fn floor() {
    assert_eq!(eval("3.7.floor()"), VmValue::Int(3));
    assert_eq!(eval("3.1.floor()"), VmValue::Int(3));
}

#[test]
fn ceil() {
    assert_eq!(eval("3.2.ceil()"), VmValue::Int(4));
    assert_eq!(eval("3.9.ceil()"), VmValue::Int(4));
}

#[test]
fn to_i() {
    assert_eq!(eval("3.5.to_i()"), VmValue::Int(3));
    assert_eq!(eval("3.9.to_i()"), VmValue::Int(3));
    assert_eq!(eval("(-3.9).to_i()"), VmValue::Int(-3));
}

#[test]
fn to_s() {
    assert_eq!(eval("3.14.to_s()"), VmValue::Str("3.14".into()));
    assert_eq!(eval("1.0.to_s()"), VmValue::Str("1.0".into()));
    assert_eq!(eval("(-3.0).to_s()"), VmValue::Str("-3.0".into()));
}

#[test]
fn abs() {
    assert_eq!(eval("n = -2.5\nn.abs()"), VmValue::Float(2.5));
    assert_eq!(eval("2.5.abs()"), VmValue::Float(2.5));
}

#[test]
fn zero() {
    assert_eq!(eval("0.0.zero?()"), VmValue::Bool(true));
    assert_eq!(eval("(-0.0).zero?()"), VmValue::Bool(true));
    assert_eq!(eval("1.5.zero?()"), VmValue::Bool(false));
}
