use crate::vm::{VmError, VmValue};

fn math_arg(args: &[VmValue], method: &str, line: u32) -> Result<f64, VmError> {
    match args {
        [VmValue::Float(f)] => Ok(*f),
        [VmValue::Int(i)]   => Ok(*i as f64),
        [_] => Err(VmError::TypeError {
            message: format!("Math.{}: argument must be numeric", method), line,
        }),
        _ => Err(VmError::TypeError {
            message: format!("Math.{} expects 1 argument, got {}", method, args.len()), line,
        }),
    }
}

/// Native dispatch for `Math` class methods.
pub fn dispatch_math_class_method(name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    match name {
        "sin"  => math_arg(args, name, line).map(|f| VmValue::Float(f.sin())),
        "cos"  => math_arg(args, name, line).map(|f| VmValue::Float(f.cos())),
        "asin" => math_arg(args, name, line).map(|f| VmValue::Float(f.asin())),
        "atan" => math_arg(args, name, line).map(|f| VmValue::Float(f.atan())),
        _ => Err(VmError::TypeError {
            message: format!("Math has no class method '{}'", name),
            line,
        }),
    }
}
