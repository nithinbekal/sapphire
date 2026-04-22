use crate::vm::{VmError, VmValue};

pub const NATIVE_METHOD_NAMES: &[&str] = &["inspect", "nil?", "to_s"];

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("inspect", 0),
    ("nil?", 0),
    ("to_s", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Nil.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Nil has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_nil_method(name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    match (name, args) {
        ("inspect", []) => Ok(VmValue::Str("nil".to_string())),
        ("nil?", []) => Ok(VmValue::Bool(true)),
        ("to_s", []) => Ok(VmValue::Str(String::new())),
        _ => Err(arg_error(name, args.len(), line)),
    }
}
