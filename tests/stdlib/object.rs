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
    assert_eq!(eval("nil.to_s()"), VmValue::Str("nil".into()));
}

#[test]
fn bool_methods() {
    assert_eq!(eval("true.to_s()"), VmValue::Str("true".into()));
    assert_eq!(eval("false.to_s()"), VmValue::Str("false".into()));
}

#[test]
fn class_method() {
    assert_eq!(
        eval("class Dog {}\nDog.new.class.name"),
        VmValue::Str("Dog".into())
    );
    assert_eq!(
        eval("42.class.name"),
        VmValue::Str("Int".into())
    );
    assert_eq!(
        eval("\"hi\".class.name"),
        VmValue::Str("String".into())
    );
}

#[test]
fn superclass_method() {
    assert_eq!(
        eval("class Animal {}\nclass Dog < Animal {}\nDog.superclass.name"),
        VmValue::Str("Animal".into())
    );
    assert_eq!(
        eval("Object.superclass"),
        VmValue::Nil
    );
    assert_eq!(
        eval("class Animal {}\nclass Dog < Animal {}\nDog.new.class.superclass.name"),
        VmValue::Str("Animal".into())
    );
}

#[test]
fn class_obj_bootstrap() {
    // Set instances return the bootstrapped ClassObj for Set.
    assert_eq!(
        eval("Set.new.class.name"),
        VmValue::Str("Set".into())
    );
    // The Set ClassObj's class is Class.
    assert_eq!(
        eval("Set.new.class.class.name"),
        VmValue::Str("Class".into())
    );
    // Class.class is Class (circular).
    assert_eq!(
        eval("Set.new.class.class.class.name"),
        VmValue::Str("Class".into())
    );
    // Object.superclass is nil (root of the hierarchy).
    assert_eq!(
        eval("Object.superclass"),
        VmValue::Nil
    );
    // Set's ClassObj is accessible directly and has the right name.
    assert_eq!(
        eval("Set.name"),
        VmValue::Str("Set".into())
    );
    // Class.name works.
    assert_eq!(
        eval("Class.name"),
        VmValue::Str("Class".into())
    );
}
