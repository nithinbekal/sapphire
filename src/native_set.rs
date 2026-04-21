use crate::gc::{GcHeap, GcRef};
use crate::vm::{format_value_with_heap, HeapObject, VmError, VmValue};

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
    let msg = METHOD_ARITIES.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Set.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Set has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_set_method(
    heap: &mut GcHeap<HeapObject>,
    r: GcRef,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match (name, args) {
        ("add", [item]) => {
            let item = item.clone();
            if !heap.get_set(r).contains(&item) {
                heap.get_set_mut(r).push(item);
            }
            Ok(recv.clone())
        }
        ("delete", [item]) => {
            let v = heap.get_set_mut(r);
            if let Some(i) = v.iter().position(|x| x == item) {
                v.remove(i);
            }
            Ok(recv.clone())
        }
        ("difference", [VmValue::Set(other_r)]) => {
            let self_items = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            let result = self_items.into_iter().filter(|x| !other.contains(x)).collect();
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        ("disjoint?", [VmValue::Set(other_r)]) => {
            let other = heap.get_set(*other_r).clone();
            Ok(VmValue::Bool(!heap.get_set(r).iter().any(|x| other.contains(x))))
        }
        ("empty?", [])    => Ok(VmValue::Bool(heap.get_set(r).is_empty())),
        ("include?", [item]) => Ok(VmValue::Bool(heap.get_set(r).contains(item))),
        ("intersection", [VmValue::Set(other_r)]) => {
            let self_items = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            let result = self_items.into_iter().filter(|x| other.contains(x)).collect();
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        ("size", [])      => Ok(VmValue::Int(heap.get_set(r).len() as i64)),
        ("subset?", [VmValue::Set(other_r)]) => {
            let other = heap.get_set(*other_r).clone();
            Ok(VmValue::Bool(heap.get_set(r).iter().all(|x| other.contains(x))))
        }
        ("superset?", [VmValue::Set(other_r)]) => {
            let self_items = heap.get_set(r).clone();
            Ok(VmValue::Bool(heap.get_set(*other_r).iter().all(|x| self_items.contains(x))))
        }
        ("to_a", []) => {
            let items = heap.get_set(r).clone();
            Ok(VmValue::List(heap.alloc(HeapObject::List(items))))
        }
        ("to_s", [])      => Ok(VmValue::Str(format_value_with_heap(heap, recv))),
        ("union", [VmValue::Set(other_r)]) => {
            let mut result = heap.get_set(r).clone();
            let other = heap.get_set(*other_r).clone();
            for item in other {
                if !result.contains(&item) { result.push(item); }
            }
            Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
        }
        (m @ ("difference" | "disjoint?" | "intersection" | "subset?" | "superset?" | "union"), [_]) =>
            Err(VmError::TypeError { message: format!("{m} expects a Set"), line }),
        _ => Err(arg_error(name, args.len(), line)),
    }
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
