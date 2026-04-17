use super::{VmValue, eval};

#[test]
fn size() {
    assert_eq!(eval("m = {a: 1, b: 2}\nm.size()"), VmValue::Int(2));
    assert_eq!(eval("{}.size()"), VmValue::Int(0));
}

#[test]
fn has_key() {
    assert_eq!(eval("m = {a: 1}\nm.has_key?(\"a\")"), VmValue::Bool(true));
    assert_eq!(eval("m = {a: 1}\nm.has_key?(\"z\")"), VmValue::Bool(false));
}

#[test]
fn delete() {
    assert_eq!(eval("m = {a: 1}\nm.delete(\"a\")"), VmValue::Int(1));
    assert_eq!(
        eval("m = {a: 1}\nm.delete(\"a\")\nm.size()"),
        VmValue::Int(0)
    );
}

#[test]
fn merge() {
    let src = r#"a = { x: 1 }
b = { y: 2 }
c = a.merge(b)
c.size()"#;
    assert_eq!(eval(src), VmValue::Int(2));

    let src2 = r#"a = { x: 1 }
b = { y: 2 }
c = a.merge(b)
c["x"]"#;
    assert_eq!(eval(src2), VmValue::Int(1));
}

#[test]
fn select() {
    let src = r#"m = { a: 1, b: 2, c: 3 }
result = m.select() { |k, v| v > 1 }
result.size()"#;
    assert_eq!(eval(src), VmValue::Int(2));

    let src2 = r#"m = { a: 1, b: 2, c: 3 }
result = m.select() { |k, v| v > 1 }
result.has_key?("a")"#;
    assert_eq!(eval(src2), VmValue::Bool(false));
}
