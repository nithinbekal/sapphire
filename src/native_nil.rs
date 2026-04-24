use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, VmError, VmValue};

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("inspect", 0),
    ("nil?", 0),
    ("to_s", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Nil.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Nil has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn nil_inspect(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [] => Ok(VmValue::Str("nil".to_string())),
        _ => Err(arg_error("inspect", args.len(), line)),
    }
}

pub fn nil_nil_q(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [] => Ok(VmValue::Bool(true)),
        _ => Err(arg_error("nil?", args.len(), line)),
    }
}

pub fn nil_to_s(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [] => Ok(VmValue::Str(String::new())),
        _ => Err(arg_error("to_s", args.len(), line)),
    }
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "inspect", 0, nil_inspect);
    define_native_method(heap, class_ref, "nil?", 0, nil_nil_q);
    define_native_method(heap, class_ref, "to_s", 0, nil_to_s);
}
