use crate::vm::{VmError, VmValue};
use VmValue::Str;

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("exist?", 1),
    ("read", 1),
    ("write", 2),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("File.{name} expects {arity} String argument(s), got {argc}"))
        .unwrap_or_else(|| format!("File has no class method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_file_class_method(name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    match (name, args) {
        ("exist?", [Str(path)]) =>
            Ok(VmValue::Bool(std::path::Path::new(path.as_str()).exists())),

        ("read", [Str(path)]) =>
            std::fs::read_to_string(path.as_str())
                .map(Str)
                .map_err(|e| VmError::Raised(Str(format!("{path}: {e}")))),

        ("write", [Str(path), Str(content)]) =>
            std::fs::write(path.as_str(), content.as_str())
                .map(|_| VmValue::Nil)
                .map_err(|e| VmError::Raised(Str(format!("{path}: {e}")))),

        _ => Err(arg_error(name, args.len(), line)),
    }
}
