use crate::gc::{GcHeap, GcRef};
use crate::vm::{format_value_with_heap, HeapObject, VmError, VmValue};

pub fn dispatch_set_method(
    heap: &mut GcHeap<HeapObject>,
    r: GcRef,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match name {
        "size" if args.is_empty() => Ok(VmValue::Int(heap.get_set(r).len() as i64)),
        "empty?" if args.is_empty() => Ok(VmValue::Bool(heap.get_set(r).is_empty())),
        "include?" if args.len() == 1 => {
            Ok(VmValue::Bool(heap.get_set(r).contains(&args[0])))
        }
        "add" if args.len() == 1 => {
            let item = args[0].clone();
            if !heap.get_set(r).contains(&item) {
                heap.get_set_mut(r).push(item);
            }
            Ok(recv.clone())
        }
        "delete" if args.len() == 1 => {
            let v = heap.get_set_mut(r);
            let pos = v.iter().position(|x| x == &args[0]);
            if let Some(i) = pos {
                v.remove(i);
            }
            Ok(recv.clone())
        }
        "to_a" if args.is_empty() => {
            let items = heap.get_set(r).clone();
            Ok(VmValue::List(heap.alloc(HeapObject::List(items))))
        }
        "to_s" if args.is_empty() => Ok(VmValue::Str(format_value_with_heap(heap, recv))),
        "union" if args.len() == 1 => match &args[0] {
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
            _ => Err(type_err("union expects a Set")),
        },
        "intersection" if args.len() == 1 => match &args[0] {
            VmValue::Set(other_r) => {
                let self_items = heap.get_set(r).clone();
                let other = heap.get_set(*other_r).clone();
                let result: Vec<VmValue> =
                    self_items.into_iter().filter(|x| other.contains(x)).collect();
                Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
            }
            _ => Err(type_err("intersection expects a Set")),
        },
        "difference" if args.len() == 1 => match &args[0] {
            VmValue::Set(other_r) => {
                let self_items = heap.get_set(r).clone();
                let other = heap.get_set(*other_r).clone();
                let result: Vec<VmValue> =
                    self_items.into_iter().filter(|x| !other.contains(x)).collect();
                Ok(VmValue::Set(heap.alloc(HeapObject::Set(result))))
            }
            _ => Err(type_err("difference expects a Set")),
        },
        "subset?" if args.len() == 1 => match &args[0] {
            VmValue::Set(other_r) => {
                let other = heap.get_set(*other_r).clone();
                let is_subset = heap.get_set(r).iter().all(|x| other.contains(x));
                Ok(VmValue::Bool(is_subset))
            }
            _ => Err(type_err("subset? expects a Set")),
        },
        "superset?" if args.len() == 1 => match &args[0] {
            VmValue::Set(other_r) => {
                let self_items = heap.get_set(r).clone();
                let is_superset = heap.get_set(*other_r).iter().all(|x| self_items.contains(x));
                Ok(VmValue::Bool(is_superset))
            }
            _ => Err(type_err("superset? expects a Set")),
        },
        "disjoint?" if args.len() == 1 => match &args[0] {
            VmValue::Set(other_r) => {
                let other = heap.get_set(*other_r).clone();
                let is_disjoint = !heap.get_set(r).iter().any(|x| other.contains(x));
                Ok(VmValue::Bool(is_disjoint))
            }
            _ => Err(type_err("disjoint? expects a Set")),
        },
        _ => Err(type_err(&format!("Set has no method '{}'", name))),
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
