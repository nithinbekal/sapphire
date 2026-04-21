use crate::vm::{VmError, VmValue};
use VmValue::Str;

/// Intermediate result type so vm.rs can do GC heap allocation for List/Instance.
pub enum ProcessResult {
    Primitive(VmValue),
    List(Vec<VmValue>),
    RunOutput { stdout: String, stderr: String, exit_code: i64 },
}

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("args", 0),
    ("pid", 0),
    ("run", 1),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Process.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Process has no class method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_process_class_method(
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<ProcessResult, VmError> {
    match (name, args) {
        ("args", []) => {
            // argv[0]=binary argv[1]="run" argv[2]=script; user args start at 3
            let list: Vec<VmValue> = std::env::args().skip(3).map(Str).collect();
            Ok(ProcessResult::List(list))
        }

        ("exit", []) => std::process::exit(0),
        ("exit", [VmValue::Int(n)]) => std::process::exit(*n as i32),
        ("exit", [_]) => Err(VmError::TypeError {
            message: "Process.exit: exit code must be an integer".to_string(), line,
        }),
        ("exit", _) => Err(VmError::TypeError {
            message: format!("Process.exit expects 0 or 1 argument, got {}", args.len()), line,
        }),

        ("pid", []) => Ok(ProcessResult::Primitive(VmValue::Int(std::process::id() as i64))),

        ("run", [Str(cmd)]) => {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(cmd.as_str())
                .output()
                .map_err(|e| VmError::Raised(Str(format!("Process.run failed: {e}"))))?;
            Ok(ProcessResult::RunOutput {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                exit_code: output.status.code().unwrap_or(-1) as i64,
            })
        }
        ("run", [_]) => Err(VmError::TypeError {
            message: "Process.run: command must be a string".to_string(), line,
        }),

        _ => Err(arg_error(name, args.len(), line)),
    }
}
