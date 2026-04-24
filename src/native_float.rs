use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, VmError, VmValue};

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

fn float_n(recv: &VmValue) -> f64 {
    match recv {
        VmValue::Float(n) => *n,
        _ => unreachable!("Float native on non-Float"),
    }
}

pub fn float_ceil(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    match args {
        [] => Ok(VmValue::Int(n.ceil() as i64)),
        _ => Err(arg_error("ceil", args.len(), line)),
    }
}

pub fn float_floor(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    match args {
        [] => Ok(VmValue::Int(n.floor() as i64)),
        _ => Err(arg_error("floor", args.len(), line)),
    }
}

pub fn float_infinite(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    match args {
        [] => Ok(VmValue::Bool(n.is_infinite())),
        _ => Err(arg_error("infinite?", args.len(), line)),
    }
}

pub fn float_nan(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    match args {
        [] => Ok(VmValue::Bool(n.is_nan())),
        _ => Err(arg_error("nan?", args.len(), line)),
    }
}

pub fn float_round(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    match args {
        [] => Ok(VmValue::Int(n.round() as i64)),
        _ => Err(arg_error("round", args.len(), line)),
    }
}

pub fn float_sqrt(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    match args {
        [] => Ok(VmValue::Float(n.sqrt())),
        _ => Err(arg_error("sqrt", args.len(), line)),
    }
}

pub fn float_to_i(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    match args {
        [] => Ok(VmValue::Int(n as i64)),
        _ => Err(arg_error("to_i", args.len(), line)),
    }
}

pub fn float_to_s(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    match args {
        [] => Ok(VmValue::Str(if n.fract() == 0.0 {
            format!("{}.0", n as i64)
        } else {
            format!("{}", n)
        })),
        _ => Err(arg_error("to_s", args.len(), line)),
    }
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "ceil", 0, float_ceil);
    define_native_method(heap, class_ref, "floor", 0, float_floor);
    define_native_method(heap, class_ref, "infinite?", 0, float_infinite);
    define_native_method(heap, class_ref, "nan?", 0, float_nan);
    define_native_method(heap, class_ref, "round", 0, float_round);
    define_native_method(heap, class_ref, "sqrt", 0, float_sqrt);
    define_native_method(heap, class_ref, "to_i", 0, float_to_i);
    define_native_method(heap, class_ref, "to_s", 0, float_to_s);
}
