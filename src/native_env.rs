use crate::vm::{VmError, VmValue};
use VmValue::Str;

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("delete", 1),
    ("fetch", 1),
    ("get", 1),
    ("set", 2),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Env.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Env has no class method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_env_class_method(name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    match (name, args) {
        ("delete", [Str(var_name)]) => {
            // SAFETY: only called from single-threaded Sapphire VM
            unsafe { std::env::remove_var(var_name.as_str()) };
            Ok(VmValue::Nil)
        }
        ("delete", [_]) => Err(VmError::TypeError {
            message: "Env.delete: name must be a string".to_string(), line,
        }),

        ("fetch", [Str(var_name)]) =>
            std::env::var(var_name.as_str()).map(Str).map_err(|_| {
                VmError::Raised(Str(format!("environment variable not found: {var_name}")))
            }),
        ("fetch", [_]) => Err(VmError::TypeError {
            message: "Env.fetch: name must be a string".to_string(), line,
        }),

        ("get", [Str(var_name)]) =>
            Ok(std::env::var(var_name.as_str()).map(Str).unwrap_or(VmValue::Nil)),
        ("get", [_]) => Err(VmError::TypeError {
            message: "Env.get: name must be a string".to_string(), line,
        }),

        ("set", [Str(var_name), Str(value)]) => {
            // SAFETY: only called from single-threaded Sapphire VM
            unsafe { std::env::set_var(var_name.as_str(), value.as_str()) };
            Ok(VmValue::Nil)
        }
        ("set", [_, _]) => Err(VmError::TypeError {
            message: "Env.set: name and value must be strings".to_string(), line,
        }),

        _ => Err(arg_error(name, args.len(), line)),
    }
}
