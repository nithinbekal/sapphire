use crate::vm::{VmError, VmValue};

/// Native dispatch for `File` class methods: `read`, `write`, `exist?`.
pub fn dispatch_file_class_method(name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    match name {
        "read" => {
            let path = match args {
                [VmValue::Str(s)] => s.clone(),
                [_] => {
                    return Err(VmError::TypeError {
                        message: "File.read: path must be a string".to_string(),
                        line,
                    });
                }
                _ => {
                    return Err(VmError::TypeError {
                        message: format!("File.read expects 1 argument, got {}", args.len()),
                        line,
                    });
                }
            };
            std::fs::read_to_string(&path)
                .map(VmValue::Str)
                .map_err(|e| VmError::Raised(VmValue::Str(format!("{}: {}", path, e))))
        }
        "write" => {
            let (path, content) = match args {
                [VmValue::Str(p), VmValue::Str(c)] => (p.clone(), c.clone()),
                [_, _] => {
                    return Err(VmError::TypeError {
                        message: "File.write: path and content must be strings".to_string(),
                        line,
                    });
                }
                _ => {
                    return Err(VmError::TypeError {
                        message: format!("File.write expects 2 arguments, got {}", args.len()),
                        line,
                    });
                }
            };
            std::fs::write(&path, content)
                .map(|_| VmValue::Nil)
                .map_err(|e| VmError::Raised(VmValue::Str(format!("{}: {}", path, e))))
        }
        "exist?" => {
            let path = match args {
                [VmValue::Str(s)] => s.clone(),
                [_] => {
                    return Err(VmError::TypeError {
                        message: "File.exist?: path must be a string".to_string(),
                        line,
                    });
                }
                _ => {
                    return Err(VmError::TypeError {
                        message: format!("File.exist? expects 1 argument, got {}", args.len()),
                        line,
                    });
                }
            };
            Ok(VmValue::Bool(std::path::Path::new(&path).exists()))
        }
        _ => Err(VmError::TypeError {
            message: format!("File has no class method '{}'", name),
            line,
        }),
    }
}
