use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, format_value_with_heap, HeapObject, VmError, VmValue};

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("add", 1),
    ("delete", 1),
    ("difference", 1),
    ("disjoint?", 1),
    ("empty?", 0),
    ("include?", 1),
    ("intersection", 1),
    ("size", 0),
    ("subset?", 1),
    ("superset?", 1),
    ("to_a", 0),
    ("to_s", 0),
    ("union", 1),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Set.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Set has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

fn set_r(recv: &VmValue) -> GcRef {
    match recv {
        VmValue::Set(r) => *r,
        _ => unreachable!("Set native on non-Set"),
    }
}

pub fn set_add(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [item] => {
            let item = item.clone();
            if !heap.get_set(r).contains(&item) {
                heap.get_set_mut(r).push(item);
            }
            Ok(recv.clone())
        }
        _ => Err(arg_error("add", args.len(), line)),
    }
}

pub fn set_delete(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [item] => {
            let v = heap.get_set_mut(r);
            if let Some(i) = v.iter().position(|x| x == item) {
                v.remove(i);
            }
            Ok(recv.clone())
        }
        _ => Err(arg_error("delete", args.len(), line)),
    }
}

pub fn set_difference(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [VmValue::Set(other_r)] => {
            let self_items = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            let result = self_items.into_iter().filter(|x| !other.contains(x)).collect();
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        [_] => Err(VmError::TypeError {
            message: "difference expects a Set".into(),
            line,
        }),
        _ => Err(arg_error("difference", args.len(), line)),
    }
}

pub fn set_disjoint(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [VmValue::Set(other_r)] => {
            let other = heap.get_set(*other_r).clone();
            Ok(VmValue::Bool(!heap.get_set(r).iter().any(|x| other.contains(x))))
        }
        [_] => Err(VmError::TypeError {
            message: "disjoint? expects a Set".into(),
            line,
        }),
        _ => Err(arg_error("disjoint?", args.len(), line)),
    }
}

pub fn set_empty(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [] => Ok(VmValue::Bool(heap.get_set(r).is_empty())),
        _ => Err(arg_error("empty?", args.len(), line)),
    }
}

pub fn set_include(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [item] => Ok(VmValue::Bool(heap.get_set(r).contains(item))),
        _ => Err(arg_error("include?", args.len(), line)),
    }
}

pub fn set_intersection(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [VmValue::Set(other_r)] => {
            let self_items = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            let result = self_items.into_iter().filter(|x| other.contains(x)).collect();
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        [_] => Err(VmError::TypeError {
            message: "intersection expects a Set".into(),
            line,
        }),
        _ => Err(arg_error("intersection", args.len(), line)),
    }
}

pub fn set_size(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [] => Ok(VmValue::Int(heap.get_set(r).len() as i64)),
        _ => Err(arg_error("size", args.len(), line)),
    }
}

pub fn set_subset(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [VmValue::Set(other_r)] => {
            let other = heap.get_set(*other_r).clone();
            Ok(VmValue::Bool(heap.get_set(r).iter().all(|x| other.contains(x))))
        }
        [_] => Err(VmError::TypeError {
            message: "subset? expects a Set".into(),
            line,
        }),
        _ => Err(arg_error("subset?", args.len(), line)),
    }
}

pub fn set_superset(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [VmValue::Set(other_r)] => {
            let self_items = heap.get_set(r).clone();
            Ok(VmValue::Bool(
                heap.get_set(*other_r)
                    .iter()
                    .all(|x| self_items.contains(x)),
            ))
        }
        [_] => Err(VmError::TypeError {
            message: "superset? expects a Set".into(),
            line,
        }),
        _ => Err(arg_error("superset?", args.len(), line)),
    }
}

pub fn set_to_a(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [] => {
            let items = heap.get_set(r).clone();
            Ok(VmValue::List(heap.alloc(HeapObject::List(items))))
        }
        _ => Err(arg_error("to_a", args.len(), line)),
    }
}

pub fn set_to_s(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let _ = set_r(recv);
    match args {
        [] => Ok(VmValue::Str(format_value_with_heap(heap, recv))),
        _ => Err(arg_error("to_s", args.len(), line)),
    }
}

pub fn set_union(heap: &mut GcHeap<HeapObject>, recv: &VmValue, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let r = set_r(recv);
    match args {
        [VmValue::Set(other_r)] => {
            let mut result = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            for item in other {
                if !result.contains(&item) {
                    result.push(item);
                }
            }
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        [_] => Err(VmError::TypeError {
            message: "union expects a Set".into(),
            line,
        }),
        _ => Err(arg_error("union", args.len(), line)),
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
