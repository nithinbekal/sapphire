use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, VmError, VmValue};

pub fn nil_inspect(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str("nil".to_string()))
}

pub fn nil_nil_q(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Bool(true))
}

pub fn nil_to_s(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(String::new()))
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "inspect", 0, nil_inspect);
    define_native_method(heap, class_ref, "nil?", 0, nil_nil_q);
    define_native_method(heap, class_ref, "to_s", 0, nil_to_s);
}
