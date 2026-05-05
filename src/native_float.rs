use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, VmError, VmValue};

fn float_n(recv: &VmValue) -> f64 {
    match recv {
        VmValue::Float(n) => *n,
        _ => unreachable!("Float native on non-Float"),
    }
}

pub fn float_ceil(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    Ok(VmValue::Int(n.ceil() as i64))
}

pub fn float_floor(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    Ok(VmValue::Int(n.floor() as i64))
}

pub fn float_infinite(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    Ok(VmValue::Bool(n.is_infinite()))
}

pub fn float_nan(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    Ok(VmValue::Bool(n.is_nan()))
}

pub fn float_round(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    Ok(VmValue::Int(n.round() as i64))
}

pub fn float_sqrt(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    Ok(VmValue::Float(n.sqrt()))
}

pub fn float_to_i(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    Ok(VmValue::Int(n as i64))
}

pub fn float_to_s(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = float_n(recv);
    Ok(VmValue::Str(if n.fract() == 0.0 {
        format!("{}.0", n as i64)
    } else {
        format!("{}", n)
    }))
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
