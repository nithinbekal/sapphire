use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, VmError, VmValue};

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
    let e = match args[0] {
        VmValue::Int(e) if e >= 0 => e as u32,
        _ => {
            return Err(VmError::TypeError {
                message: "Int.pow expects a non-negative integer exponent".to_string(),
                line,
            });
        }
    };
    Ok(VmValue::Int(n.pow(e)))
}

pub fn int_to_f(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = int_n(recv);
    Ok(VmValue::Float(n as f64))
}

pub fn int_to_s(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let n = int_n(recv);
    Ok(VmValue::Str(n.to_string()))
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "pow", 1, int_pow);
    define_native_method(heap, class_ref, "to_f", 0, int_to_f);
    define_native_method(heap, class_ref, "to_s", 0, int_to_s);
}
