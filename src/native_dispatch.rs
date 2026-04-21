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
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match recv {
        VmValue::Int(n) => dispatch_int_method(*n, name, args, line),
        VmValue::Float(n) => dispatch_float_method(*n, name, args, line),
        VmValue::Str(s) => crate::native_string::dispatch_str_method(heap, s, name, args, line),
        VmValue::Bool(b) => dispatch_bool_method(*b, name, args, line),
        VmValue::Nil => dispatch_nil_method(name, args, line),
        VmValue::List(r) => crate::native_list::dispatch_list_method(heap, *r, recv, name, args, line),
        VmValue::Map(r) => crate::native_map::dispatch_map_method(heap, *r, recv, name, args, line),
        VmValue::Set(r) => crate::native_set::dispatch_set_method(heap, *r, recv, name, args, line),
        VmValue::Range { from, to } => {
            dispatch_range_method(heap, *from, *to, recv, name, args, line)
        }
        other => Err(VmError::TypeError {
            message: format!("'{}' has no method '{}'", other, name),
            line,
        }),
    }
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

// ── Per-type dispatch ─────────────────────────────────────────────────────────

fn dispatch_int_method(n: i64, name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match (name, args) {
        ("to_s", []) => Ok(VmValue::Str(n.to_string())),
        ("to_f", []) => Ok(VmValue::Float(n as f64)),
        ("pow", [VmValue::Int(e)]) if *e >= 0 => Ok(VmValue::Int(n.pow(*e as u32))),
        _ => Err(type_err(&format!("Int has no method '{}'", name))),
    }
}

fn dispatch_float_method(n: f64, name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match (name, args) {
        ("to_s", []) => Ok(VmValue::Str(if n.fract() == 0.0 {
            format!("{}.0", n as i64)
        } else {
            format!("{}", n)
        })),
        ("to_i", []) => Ok(VmValue::Int(n as i64)),
        ("round", []) => Ok(VmValue::Int(n.round() as i64)),
        ("floor", []) => Ok(VmValue::Int(n.floor() as i64)),
        ("ceil", []) => Ok(VmValue::Int(n.ceil() as i64)),
        ("sqrt", []) => Ok(VmValue::Float(n.sqrt())),
        ("nan?", []) => Ok(VmValue::Bool(n.is_nan())),
        ("infinite?", []) => Ok(VmValue::Bool(n.is_infinite())),
        _ => Err(type_err(&format!("Float has no method '{}'", name))),
    }
}

fn dispatch_bool_method(b: bool, name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match (name, args) {
        ("to_s", []) => Ok(VmValue::Str(b.to_string())),
        ("nil?", []) => Ok(VmValue::Bool(false)),
        _ => Err(type_err(&format!("Bool has no method '{}'", name))),
    }
}

fn dispatch_nil_method(name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match (name, args) {
        ("to_s", []) => Ok(VmValue::Str(String::new())),
        ("nil?", []) => Ok(VmValue::Bool(true)),
        ("inspect", []) => Ok(VmValue::Str("nil".to_string())),
        _ => Err(type_err(&format!("Nil has no method '{}'", name))),
    }
}

fn dispatch_range_method(
    heap: &mut GcHeap<HeapObject>,
    from: i64,
    to: i64,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match name {
        "size" if args.is_empty() => Ok(VmValue::Int((to - from).max(0))),
        "to_a" if args.is_empty() => {
            let v: Vec<VmValue> = (from..to).map(VmValue::Int).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        "include?" if args.len() == 1 => match &args[0] {
            VmValue::Int(n) => Ok(VmValue::Bool(n >= &from && n < &to)),
            _ => Err(type_err("include? expects an Int")),
        },
        "first" if args.is_empty() => Ok(VmValue::Int(from)),
        "last" if args.is_empty() => Ok(VmValue::Int(to - 1)),
        "min" if args.is_empty() => Ok(VmValue::Int(from)),
        "max" if args.is_empty() => Ok(VmValue::Int(to - 1)),
        "to_s" if args.is_empty() => Ok(VmValue::Str(format!("{}", recv))),
        _ => Err(VmError::TypeError {
            message: format!("Range has no method '{}'", name),
            line,
        }),
    }
}
