use super::{VmValue, eval};

#[test]
fn zero() {
    assert_eq!(eval("0.zero?()"), VmValue::Bool(true));
    assert_eq!(eval("1.zero?()"), VmValue::Bool(false));
}

#[test]
fn positive() {
    assert_eq!(eval("3.positive?()"), VmValue::Bool(true));
    assert_eq!(eval("(-1).positive?()"), VmValue::Bool(false));
}

#[test]
fn negative() {
    assert_eq!(eval("(-1.0).negative?()"), VmValue::Bool(true));
    assert_eq!(eval("1.negative?()"), VmValue::Bool(false));
}

#[test]
fn clamp() {
    assert_eq!(eval("10.clamp(1, 5)"), VmValue::Int(5));
    assert_eq!(eval("0.clamp(1, 5)"), VmValue::Int(1));
    assert_eq!(eval("3.clamp(1, 5)"), VmValue::Int(3));
}

#[test]
fn type_annotation_accepts_int_and_float() {
    let src = "def double(x: Num) { x + x }\ndouble(3)";
    assert_eq!(eval(src), VmValue::Int(6));

    let src2 = "def double(x: Num) { x + x }\ndouble(1.5)";
    assert_eq!(eval(src2), VmValue::Float(3.0));
}
