use crate::vm::{VmError, VmValue};

pub const NATIVE_METHOD_NAMES: &[&str] = &[
    "ceil", "floor", "infinite?", "nan?", "round", "sqrt", "to_i", "to_s",
];

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("ceil", 0),
    ("floor", 0),
    ("infinite?", 0),
    ("nan?", 0),
    ("round", 0),
    ("sqrt", 0),
    ("to_i", 0),
    ("to_s", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Float.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Float has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_float_method(
    n: f64,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match (name, args) {
        ("ceil", []) => Ok(VmValue::Int(n.ceil() as i64)),
        ("floor", []) => Ok(VmValue::Int(n.floor() as i64)),
        ("infinite?", []) => Ok(VmValue::Bool(n.is_infinite())),
        ("nan?", []) => Ok(VmValue::Bool(n.is_nan())),
        ("round", []) => Ok(VmValue::Int(n.round() as i64)),
        ("sqrt", []) => Ok(VmValue::Float(n.sqrt())),
        ("to_i", []) => Ok(VmValue::Int(n as i64)),
        ("to_s", []) => Ok(VmValue::Str(if n.fract() == 0.0 {
            format!("{}.0", n as i64)
        } else {
            format!("{}", n)
        })),
        _ => Err(arg_error(name, args.len(), line)),
    }
}
