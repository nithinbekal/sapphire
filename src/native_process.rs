use std::collections::HashMap;
use crate::vm::{VmError, VmValue};

/// Intermediate result type so vm.rs can do GC heap allocation for List/Map.
pub enum ProcessResult {
    Primitive(VmValue),
    List(Vec<VmValue>),
    Map(HashMap<String, VmValue>),
}

pub fn dispatch_process_class_method(
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<ProcessResult, VmError> {
    match name {
        "pid" => {
            if !args.is_empty() {
                return Err(VmError::TypeError {
                    message: format!("Process.pid takes no arguments, got {}", args.len()),
                    line,
                });
            }
            Ok(ProcessResult::Primitive(VmValue::Int(
                std::process::id() as i64,
            )))
        }
        "exit" => {
            let code: i32 = match args {
                [] => 0,
                [VmValue::Int(n)] => *n as i32,
                [_] => {
                    return Err(VmError::TypeError {
                        message: "Process.exit: exit code must be an integer".to_string(),
                        line,
                    });
                }
                _ => {
                    return Err(VmError::TypeError {
                        message: format!(
                            "Process.exit expects 0 or 1 argument, got {}",
                            args.len()
                        ),
                        line,
                    });
                }
            };
            std::process::exit(code);
        }
        "args" => {
            if !args.is_empty() {
                return Err(VmError::TypeError {
                    message: format!("Process.args takes no arguments, got {}", args.len()),
                    line,
                });
            }
            // argv[0]=binary argv[1]="run" argv[2]=script; user args start at 3
            let list: Vec<VmValue> = std::env::args().skip(3).map(VmValue::Str).collect();
            Ok(ProcessResult::List(list))
        }
        "run" => {
            let cmd = match args {
                [VmValue::Str(s)] => s.clone(),
                [_] => {
                    return Err(VmError::TypeError {
                        message: "Process.run: command must be a string".to_string(),
                        line,
                    });
                }
                _ => {
                    return Err(VmError::TypeError {
                        message: format!("Process.run expects 1 argument, got {}", args.len()),
                        line,
                    });
                }
            };
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .map_err(|e| {
                    VmError::Raised(VmValue::Str(format!("Process.run failed: {}", e)))
                })?;
            let mut map = HashMap::new();
            map.insert(
                "stdout".to_string(),
                VmValue::Str(String::from_utf8_lossy(&output.stdout).into_owned()),
            );
            map.insert(
                "stderr".to_string(),
                VmValue::Str(String::from_utf8_lossy(&output.stderr).into_owned()),
            );
            map.insert(
                "exit_code".to_string(),
                VmValue::Int(output.status.code().unwrap_or(-1) as i64),
            );
            Ok(ProcessResult::Map(map))
        }
        _ => Err(VmError::TypeError {
            message: format!("Process has no class method '{}'", name),
            line,
        }),
    }
}
