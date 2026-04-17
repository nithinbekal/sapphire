use super::{VmValue, eval};

#[test]
fn is_a_instance_hierarchy() {
    let base = "class Animal { attr name }\nclass Dog < Animal { attr breed }\nd = Dog.new(name: \"Rex\", breed: \"Lab\")\n";

    assert_eq!(
        eval(&(base.to_string() + "d.is_a?(Dog)")),
        VmValue::Bool(true)
    );
    assert_eq!(
        eval(&(base.to_string() + "d.is_a?(Animal)")),
        VmValue::Bool(true)
    );
}

#[test]
fn is_a_unrelated_class() {
    let src = "class Animal { attr name }\nclass Dog < Animal { attr breed }\nclass Cat {}\nd = Dog.new(name: \"Rex\", breed: \"Lab\")\n";
    assert_eq!(
        eval(&(src.to_string() + "d.is_a?(Cat)")),
        VmValue::Bool(false)
    );
}

#[test]
fn nil_methods() {
    assert_eq!(eval("nil.nil?()"), VmValue::Bool(true));
    assert_eq!(eval("false.nil?()"), VmValue::Bool(false));
    assert_eq!(eval("nil.to_s()"), VmValue::Str("".into()));
}

#[test]
fn bool_methods() {
    assert_eq!(eval("true.to_s()"), VmValue::Str("true".into()));
    assert_eq!(eval("false.to_s()"), VmValue::Str("false".into()));
}
