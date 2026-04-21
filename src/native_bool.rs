use crate::vm::{VmError, VmValue};

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("nil?", 0),
    ("to_s", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Bool.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Bool has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_bool_method(b: bool, name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    match (name, args) {
        ("nil?", []) => Ok(VmValue::Bool(false)),
        ("to_s", []) => Ok(VmValue::Str(b.to_string())),
        _ => Err(arg_error(name, args.len(), line)),
    }
}
