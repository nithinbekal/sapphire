use crate::vm::{VmError, VmValue};

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("pow", 1),
    ("to_f", 0),
    ("to_s", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Int.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Int has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_int_method(n: i64, name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    match (name, args) {
        ("pow", [VmValue::Int(e)]) if *e >= 0 => Ok(VmValue::Int(n.pow(*e as u32))),
        ("to_f", []) => Ok(VmValue::Float(n as f64)),
        ("to_s", []) => Ok(VmValue::Str(n.to_string())),
        ("pow", _) => Err(VmError::TypeError {
            message: "Int has no method 'pow'".to_string(),
            line,
        }),
        _ => Err(arg_error(name, args.len(), line)),
    }
}
