use super::{VmValue, eval};

#[test]
fn each() {
    let src = "sum = 0\n(1..4).each() { |i| sum = sum + i }\nsum";
    assert_eq!(eval(src), VmValue::Int(6)); // 1 + 2 + 3 (exclusive upper bound)
}

#[test]
fn to_a() {
    let src = "r = 1..4\nr.to_a().size()";
    assert_eq!(eval(src), VmValue::Int(3));
}

#[test]
fn include() {
    // VM ranges are exclusive upper bound
    assert_eq!(eval("(1..10).include?(5)"), VmValue::Bool(true));
    assert_eq!(eval("(1..10).include?(1)"), VmValue::Bool(true));
    assert_eq!(eval("(1..10).include?(10)"), VmValue::Bool(false)); // exclusive
    assert_eq!(eval("(1..10).include?(11)"), VmValue::Bool(false));
}

#[test]
fn to_s() {
    assert_eq!(eval("(1..5).to_s()"), VmValue::Str("1..5".into()));
}
