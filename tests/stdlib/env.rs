use super::{VmValue, eval, eval_err};

#[test]
fn get_set_round_trip() {
    unsafe { std::env::set_var("SPR_TEST_GET", "hello") };
    assert_eq!(eval(r#"Env.get("SPR_TEST_GET")"#), VmValue::Str("hello".to_string()));
}

#[test]
fn get_missing_returns_nil() {
    assert_eq!(eval(r#"Env.get("SPR_DEFINITELY_NOT_SET_XYZ_123")"#), VmValue::Nil);
}

#[test]
fn fetch_raises_when_missing() {
    let err = eval_err(r#"Env.fetch("SPR_DEFINITELY_NOT_SET_XYZ_123")"#);
    assert!(matches!(err, sapphire::vm::VmError::Raised(_)));
}

#[test]
fn set_and_get() {
    eval(r#"Env.set("SPR_TEST_SET", "world")"#);
    assert_eq!(
        std::env::var("SPR_TEST_SET").unwrap(),
        "world"
    );
}

#[test]
fn delete() {
    unsafe { std::env::set_var("SPR_TEST_DEL", "bye") };
    assert_eq!(eval(r#"Env.delete("SPR_TEST_DEL")"#), VmValue::Nil);
    assert!(std::env::var("SPR_TEST_DEL").is_err());
}

#[test]
fn all_returns_map() {
    unsafe { std::env::set_var("SPR_TEST_ALL", "yes") };
    assert_eq!(eval(r#"Env.all().has_key?("SPR_TEST_ALL")"#), VmValue::Bool(true));
}
