use crate::gc::GcHeap;
use crate::vm::{HeapObject, VmError, VmValue};
use std::cmp::Ordering;

// ── Public utilities used throughout the VM ───────────────────────────────────

pub fn is_falsy(v: &VmValue) -> bool {
    matches!(v, VmValue::Nil | VmValue::Bool(false))
}

/// Return the stdlib class name for a primitive value, used to look up
/// compiled stdlib methods in the class registry.
pub fn primitive_class_name(val: &VmValue) -> Option<&'static str> {
    match val {
        VmValue::Int(_) => Some("Int"),
        VmValue::Float(_) => Some("Float"),
        VmValue::Str(_) => Some("String"),
        VmValue::Bool(_) => Some("Bool"),
        VmValue::Nil => Some("Nil"),
        VmValue::List(_) => Some("List"),
        VmValue::Map(_) => Some("Map"),
        VmValue::Set(_) => Some("Set"),
        _ => None,
    }
}

/// Return the type name of a value for use in runtime type-checking error messages.
pub fn value_type_name(val: &VmValue) -> &str {
    match val {
        VmValue::Int(_) => "Int",
        VmValue::Float(_) => "Float",
        VmValue::Str(_) => "String",
        VmValue::Bool(_) => "Bool",
        VmValue::Nil => "Nil",
        VmValue::List(_) => "List",
        VmValue::Map(_) => "Map",
        VmValue::Set(_) => "Set",
        VmValue::Range { .. } => "Range",
        VmValue::Instance { class_name, .. } => class_name.as_str(),
        VmValue::Class { name, .. } => name.as_str(),
        VmValue::Function(_) => "Function",
        VmValue::Closure { .. } => "Function",
        VmValue::ClassObj(_) => "Class",
    }
}

/// Simple comparison for sorting — numbers compare numerically, strings lexicographically.
pub fn vm_value_partial_cmp(a: &VmValue, b: &VmValue) -> Ordering {
    match (a, b) {
        (VmValue::Int(x), VmValue::Int(y)) => x.cmp(y),
        (VmValue::Float(x), VmValue::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (VmValue::Int(x), VmValue::Float(y)) => {
            (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        (VmValue::Float(x), VmValue::Int(y)) => {
            x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal)
        }
        (VmValue::Str(x), VmValue::Str(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

pub fn numeric_binop(
    a: &VmValue,
    b: &VmValue,
    line: u32,
    verb: &str,
    int_op: impl Fn(i64, i64) -> i64,
    float_op: impl Fn(f64, f64) -> f64,
) -> Result<VmValue, VmError> {
    match (a, b) {
        (VmValue::Int(x), VmValue::Int(y)) => Ok(VmValue::Int(int_op(*x, *y))),
        (VmValue::Float(x), VmValue::Float(y)) => Ok(VmValue::Float(float_op(*x, *y))),
        (VmValue::Int(x), VmValue::Float(y)) => Ok(VmValue::Float(float_op(*x as f64, *y))),
        (VmValue::Float(x), VmValue::Int(y)) => Ok(VmValue::Float(float_op(*x, *y as f64))),
        _ => Err(VmError::TypeError {
            message: format!("cannot {} {} and {}", verb, a, b),
            line,
        }),
    }
}

pub fn numeric_cmp(
    a: &VmValue,
    b: &VmValue,
    line: u32,
    op: impl Fn(f64, f64) -> bool,
) -> Result<bool, VmError> {
    let x = to_float(a).ok_or_else(|| VmError::TypeError {
        message: format!("cannot compare {} and {}", a, b),
        line,
    })?;
    let y = to_float(b).ok_or_else(|| VmError::TypeError {
        message: format!("cannot compare {} and {}", a, b),
        line,
    })?;
    Ok(op(x, y))
}

fn to_float(v: &VmValue) -> Option<f64> {
    match v {
        VmValue::Int(n) => Some(*n as f64),
        VmValue::Float(n) => Some(*n),
        _ => None,
    }
}

// ── Native method dispatch ────────────────────────────────────────────────────

/// Dispatch a native (non-block) method call on a built-in type.
pub fn dispatch_native_method(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    name: &str,
    _args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    Err(VmError::TypeError {
        message: format!("'{}' has no method '{}'", recv, name),
        line,
    })
}

/// Like `dispatch_native_method` but returns `None` when no native handler
/// exists for this method, allowing callers to try the class registry next.
/// Any real type error (wrong arg count, wrong type, etc.) is still `Some(Err)`.
pub fn try_native_method(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Option<Result<VmValue, VmError>> {
    match dispatch_native_method(heap, recv, name, args, line) {
        Err(VmError::TypeError { ref message, .. }) if message.contains("has no method") => None,
        result => Some(result),
    }
}
