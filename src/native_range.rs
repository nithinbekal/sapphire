use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, VmError, VmValue};

fn range_recv(recv: &VmValue) -> (i64, i64) {
    match recv {
        VmValue::Range { from, to } => (*from, *to),
        _ => unreachable!("Range native on non-Range"),
    }
}

pub fn range_first(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let (from, _to) = range_recv(recv);
    Ok(VmValue::Int(from))
}

pub fn range_include_q(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let (from, to) = range_recv(recv);
    match args {
        [VmValue::Int(n)] => Ok(VmValue::Bool(n >= &from && n < &to)),
        [_] => Err(VmError::TypeError {
            message: "include? expects an Int".to_string(),
            line,
        }),
        _ => unreachable!("Range#include?: expected 1 argument"),
    }
}

pub fn range_last(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let (_from, to) = range_recv(recv);
    Ok(VmValue::Int(to - 1))
}

pub fn range_max(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let (_from, to) = range_recv(recv);
    Ok(VmValue::Int(to - 1))
}

pub fn range_min(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let (from, _to) = range_recv(recv);
    Ok(VmValue::Int(from))
}

pub fn range_size(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let (from, to) = range_recv(recv);
    Ok(VmValue::Int((to - from).max(0)))
}

pub fn range_to_a(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let (from, to) = range_recv(recv);
    let v: Vec<VmValue> = (from..to).map(VmValue::Int).collect();
    Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
}

pub fn range_to_s(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(format!("{}", recv)))
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "first", 0, range_first);
    define_native_method(heap, class_ref, "include?", 1, range_include_q);
    define_native_method(heap, class_ref, "last", 0, range_last);
    define_native_method(heap, class_ref, "max", 0, range_max);
    define_native_method(heap, class_ref, "min", 0, range_min);
    define_native_method(heap, class_ref, "size", 0, range_size);
    define_native_method(heap, class_ref, "to_a", 0, range_to_a);
    define_native_method(heap, class_ref, "to_s", 0, range_to_s);
}
