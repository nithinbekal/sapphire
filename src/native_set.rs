use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, format_value_with_heap, HeapObject, VmError, VmValue};

fn set_r(recv: &VmValue) -> GcRef {
    match recv {
        VmValue::Set(r) => *r,
        _ => unreachable!("Set native on non-Set"),
    }
}

pub fn set_add(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    let item = args[0].clone();
    if !heap.get_set(r).contains(&item) {
        heap.get_set_mut(r).push(item);
    }
    Ok(recv.clone())
}

pub fn set_delete(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    let item = &args[0];
    let v = heap.get_set_mut(r);
    if let Some(i) = v.iter().position(|x| x == item) {
        v.remove(i);
    }
    Ok(recv.clone())
}

pub fn set_difference(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match &args[0] {
        VmValue::Set(other_r) => {
            let self_items = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            let result = self_items.into_iter().filter(|x| !other.contains(x)).collect();
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        _ => Err(VmError::TypeError {
            message: "difference expects a Set".into(),
            line,
        }),
    }
}

pub fn set_disjoint(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match &args[0] {
        VmValue::Set(other_r) => {
            let other = heap.get_set(*other_r).clone();
            Ok(VmValue::Bool(!heap.get_set(r).iter().any(|x| other.contains(x))))
        }
        _ => Err(VmError::TypeError {
            message: "disjoint? expects a Set".into(),
            line,
        }),
    }
}

pub fn set_empty(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    Ok(VmValue::Bool(heap.get_set(r).is_empty()))
}

pub fn set_include(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    Ok(VmValue::Bool(heap.get_set(r).contains(&args[0])))
}

pub fn set_intersection(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match &args[0] {
        VmValue::Set(other_r) => {
            let self_items = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            let result = self_items.into_iter().filter(|x| other.contains(x)).collect();
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        _ => Err(VmError::TypeError {
            message: "intersection expects a Set".into(),
            line,
        }),
    }
}

pub fn set_size(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    Ok(VmValue::Int(heap.get_set(r).len() as i64))
}

pub fn set_subset(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match &args[0] {
        VmValue::Set(other_r) => {
            let other = heap.get_set(*other_r).clone();
            Ok(VmValue::Bool(heap.get_set(r).iter().all(|x| other.contains(x))))
        }
        _ => Err(VmError::TypeError {
            message: "subset? expects a Set".into(),
            line,
        }),
    }
}

pub fn set_superset(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match &args[0] {
        VmValue::Set(other_r) => {
            let self_items = heap.get_set(r).clone();
            Ok(VmValue::Bool(
                heap.get_set(*other_r)
                    .iter()
                    .all(|x| self_items.contains(x)),
            ))
        }
        _ => Err(VmError::TypeError {
            message: "superset? expects a Set".into(),
            line,
        }),
    }
}

pub fn set_to_a(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    let items = heap.get_set(r).clone();
    Ok(VmValue::List(heap.alloc(HeapObject::List(items))))
}

pub fn set_to_s(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let _ = set_r(recv);
    Ok(VmValue::Str(format_value_with_heap(heap, recv)))
}

pub fn set_union(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match &args[0] {
        VmValue::Set(other_r) => {
            let mut result = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            for item in other {
                if !result.contains(&item) {
                    result.push(item);
                }
            }
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        _ => Err(VmError::TypeError {
            message: "union expects a Set".into(),
            line,
        }),
    }
}

/// Register native Set instance methods on the bootstrapped Set `ClassObject`.
pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "add", 1, set_add);
    define_native_method(heap, class_ref, "delete", 1, set_delete);
    define_native_method(heap, class_ref, "difference", 1, set_difference);
    define_native_method(heap, class_ref, "disjoint?", 1, set_disjoint);
    define_native_method(heap, class_ref, "empty?", 0, set_empty);
    define_native_method(heap, class_ref, "include?", 1, set_include);
    define_native_method(heap, class_ref, "intersection", 1, set_intersection);
    define_native_method(heap, class_ref, "size", 0, set_size);
    define_native_method(heap, class_ref, "subset?", 1, set_subset);
    define_native_method(heap, class_ref, "superset?", 1, set_superset);
    define_native_method(heap, class_ref, "to_a", 0, set_to_a);
    define_native_method(heap, class_ref, "to_s", 0, set_to_s);
    define_native_method(heap, class_ref, "union", 1, set_union);
}

/// Deduplicate a list of values, preserving insertion order.
pub fn dedup_list(items: Vec<VmValue>) -> Vec<VmValue> {
    let mut elems: Vec<VmValue> = Vec::new();
    for item in items {
        if !elems.contains(&item) {
            elems.push(item);
        }
    }
    elems
}
