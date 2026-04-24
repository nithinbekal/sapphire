use crate::gc::{GcHeap, GcRef};
use crate::native::vm_value_partial_cmp;
use crate::vm::{define_native_method, format_value_with_heap, HeapObject, VmError, VmValue};

fn map_r(recv: &VmValue) -> GcRef {
    match recv {
        VmValue::Map(r) => *r,
        _ => unreachable!("Map native on non-Map"),
    }
}

pub fn map_delete(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    match &args[0] {
        VmValue::Str(k) => Ok(heap.get_map_mut(r).remove(k.as_str()).unwrap_or(VmValue::Nil)),
        _ => Err(VmError::TypeError {
            message: "delete expects a String key".to_string(),
            line,
        }),
    }
}

pub fn map_empty_q(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    Ok(VmValue::Bool(heap.get_map(r).is_empty()))
}

pub fn map_get(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    match &args[0] {
        VmValue::Str(k) => Ok(heap.get_map(r).get(k.as_str()).cloned().unwrap_or(VmValue::Nil)),
        _ => Err(VmError::TypeError {
            message: "get expects a String key".to_string(),
            line,
        }),
    }
}

pub fn map_has_key_q(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    match &args[0] {
        VmValue::Str(k) => Ok(VmValue::Bool(heap.get_map(r).contains_key(k.as_str()))),
        _ => Err(VmError::TypeError {
            message: "has_key? expects a String".to_string(),
            line,
        }),
    }
}

pub fn map_keys(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    let mut keys: Vec<VmValue> = heap
        .get_map(r)
        .keys()
        .map(|k| VmValue::Str(k.clone()))
        .collect();
    keys.sort_by(vm_value_partial_cmp);
    Ok(VmValue::List(heap.alloc(HeapObject::List(keys))))
}

pub fn map_merge(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    match &args[0] {
        VmValue::Map(other_r) => {
            let other_r = *other_r;
            let mut new_map = heap.get_map(r).clone();
            for (k, v) in heap.get_map(other_r).iter() {
                new_map.insert(k.clone(), v.clone());
            }
            Ok(VmValue::Map(heap.alloc(HeapObject::Map(new_map))))
        }
        _ => Err(VmError::TypeError {
            message: "merge expects a Map".to_string(),
            line,
        }),
    }
}

pub fn map_set(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    match &args[0] {
        VmValue::Str(k) => {
            let k = k.clone();
            let v = args[1].clone();
            heap.get_map_mut(r).insert(k, v.clone());
            Ok(v)
        }
        _ => Err(VmError::TypeError {
            message: "set expects a String key".to_string(),
            line,
        }),
    }
}

pub fn map_size(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    Ok(VmValue::Int(heap.get_map(r).len() as i64))
}

pub fn map_to_s(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(format_value_with_heap(heap, recv)))
}

pub fn map_values(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = map_r(recv);
    let mut pairs: Vec<(String, VmValue)> = heap
        .get_map(r)
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
    let vals: Vec<VmValue> = pairs.into_iter().map(|(_, v)| v).collect();
    Ok(VmValue::List(heap.alloc(HeapObject::List(vals))))
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "delete", 1, map_delete);
    define_native_method(heap, class_ref, "empty?", 0, map_empty_q);
    define_native_method(heap, class_ref, "get", 1, map_get);
    define_native_method(heap, class_ref, "has_key?", 1, map_has_key_q);
    define_native_method(heap, class_ref, "keys", 0, map_keys);
    define_native_method(heap, class_ref, "merge", 1, map_merge);
    define_native_method(heap, class_ref, "set", 2, map_set);
    define_native_method(heap, class_ref, "size", 0, map_size);
    define_native_method(heap, class_ref, "to_s", 0, map_to_s);
    define_native_method(heap, class_ref, "values", 0, map_values);
}
