use super::{VmValue, eval};

#[test]
fn constants() {
    assert_eq!(eval("Math.PI"), VmValue::Float(std::f64::consts::PI));
    assert_eq!(eval("Math.E"), VmValue::Float(std::f64::consts::E));
}

#[test]
fn sin() {
    assert_eq!(eval("Math.sin(0)"), VmValue::Float(0.0_f64.sin()));
    assert_eq!(eval("Math.sin(Math.PI)"), VmValue::Float(std::f64::consts::PI.sin()));
}

#[test]
fn cos() {
    assert_eq!(eval("Math.cos(0)"), VmValue::Float(0.0_f64.cos()));
    assert_eq!(eval("Math.cos(Math.PI)"), VmValue::Float(std::f64::consts::PI.cos()));
}

#[test]
fn tan() {
    assert_eq!(eval("Math.tan(0)"), VmValue::Float(0.0_f64.tan()));
}

#[test]
fn asin() {
    assert_eq!(eval("Math.asin(1)"), VmValue::Float(1.0_f64.asin()));
}

#[test]
fn acos() {
    assert_eq!(eval("Math.acos(1)"), VmValue::Float(1.0_f64.acos()));
}

#[test]
fn atan() {
    assert_eq!(eval("Math.atan(1)"), VmValue::Float(1.0_f64.atan()));
}

#[test]
fn atan2() {
    assert_eq!(eval("Math.atan2(1, 1)"), VmValue::Float(1.0_f64.atan2(1.0)));
    assert_eq!(eval("Math.atan2(0, 1)"), VmValue::Float(0.0_f64.atan2(1.0)));
}
