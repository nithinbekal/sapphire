use crate::vm::{VmError, VmValue};

pub fn dispatch_env_class_method(name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    match name {
        "get" => {
            let var_name = match args {
                [VmValue::Str(s)] => s.clone(),
                [_] => return Err(VmError::TypeError {
                    message: "Env.get: name must be a string".to_string(),
                    line,
                }),
                _ => return Err(VmError::TypeError {
                    message: format!("Env.get expects 1 argument, got {}", args.len()),
                    line,
                }),
            };
            Ok(std::env::var(&var_name).map(VmValue::Str).unwrap_or(VmValue::Nil))
        }
        "fetch" => {
            let var_name = match args {
                [VmValue::Str(s)] => s.clone(),
                [_] => return Err(VmError::TypeError {
                    message: "Env.fetch: name must be a string".to_string(),
                    line,
                }),
                _ => return Err(VmError::TypeError {
                    message: format!("Env.fetch expects 1 argument, got {}", args.len()),
                    line,
                }),
            };
            std::env::var(&var_name).map(VmValue::Str).map_err(|_| {
                VmError::Raised(VmValue::Str(format!("environment variable not found: {}", var_name)))
            })
        }
        "set" => {
            let (var_name, value) = match args {
                [VmValue::Str(k), VmValue::Str(v)] => (k.clone(), v.clone()),
                [_, _] => return Err(VmError::TypeError {
                    message: "Env.set: name and value must be strings".to_string(),
                    line,
                }),
                _ => return Err(VmError::TypeError {
                    message: format!("Env.set expects 2 arguments, got {}", args.len()),
                    line,
                }),
            };
            // SAFETY: only called from single-threaded Sapphire VM
            unsafe { std::env::set_var(&var_name, &value) };
            Ok(VmValue::Nil)
        }
        "delete" => {
            let var_name = match args {
                [VmValue::Str(s)] => s.clone(),
                [_] => return Err(VmError::TypeError {
                    message: "Env.delete: name must be a string".to_string(),
                    line,
                }),
                _ => return Err(VmError::TypeError {
                    message: format!("Env.delete expects 1 argument, got {}", args.len()),
                    line,
                }),
            };
            // SAFETY: only called from single-threaded Sapphire VM
            unsafe { std::env::remove_var(&var_name) };
            Ok(VmValue::Nil)
        }
        _ => Err(VmError::TypeError {
            message: format!("Env has no class method '{}'", name),
            line,
        }),
    }
}
