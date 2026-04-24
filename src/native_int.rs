use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, VmError, VmValue};

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("pow", 1),
    ("to_f", 0),
    ("to_s", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Int.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Int has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

fn int_n(recv: &VmValue) -> i64 {
    match recv {
        VmValue::Int(n) => *n,
        _ => unreachable!("Int native on non-Int"),
    }
}

pub fn int_pow(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = int_n(recv);
    match args {
        [VmValue::Int(e)] if *e >= 0 => Ok(VmValue::Int(n.pow(*e as u32))),
        [_] => Err(VmError::TypeError {
            message: "Int has no method 'pow'".to_string(),
            line,
        }),
        _ => Err(arg_error("pow", args.len(), line)),
    }
}

pub fn int_to_f(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = int_n(recv);
    match args {
        [] => Ok(VmValue::Float(n as f64)),
        _ => Err(arg_error("to_f", args.len(), line)),
    }
}

pub fn int_to_s(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let n = int_n(recv);
    match args {
        [] => Ok(VmValue::Str(n.to_string())),
        _ => Err(arg_error("to_s", args.len(), line)),
    }
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "pow", 1, int_pow);
    define_native_method(heap, class_ref, "to_f", 0, int_to_f);
    define_native_method(heap, class_ref, "to_s", 0, int_to_s);
}
