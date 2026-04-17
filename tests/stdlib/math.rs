use super::{VmValue, eval};

#[test]
fn constants() {
    assert_eq!(eval("Math.PI"), VmValue::Float(std::f64::consts::PI));
    assert_eq!(eval("Math.E"), VmValue::Float(std::f64::consts::E));
}
