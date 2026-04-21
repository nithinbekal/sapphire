use crate::gc::{GcHeap, GcRef};
use crate::native_dispatch::vm_value_partial_cmp;
use crate::vm::{format_value_with_heap, HeapObject, VmError, VmValue};

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("delete", 1),
    ("empty?", 0),
    ("get", 1),
    ("has_key?", 1),
    ("keys", 0),
    ("merge", 1),
    ("set", 2),
    ("size", 0),
    ("to_s", 0),
    ("values", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Map.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Map has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

pub fn dispatch_map_method(
    heap: &mut GcHeap<HeapObject>,
    r: GcRef,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match (name, args) {
        ("delete", [VmValue::Str(k)]) => Ok(heap.get_map_mut(r).remove(k.as_str()).unwrap_or(VmValue::Nil)),
        ("empty?", []) => Ok(VmValue::Bool(heap.get_map(r).is_empty())),
        ("get", [VmValue::Str(k)]) => Ok(heap
            .get_map(r)
            .get(k.as_str())
            .cloned()
            .unwrap_or(VmValue::Nil)),
        ("has_key?", [VmValue::Str(k)]) => Ok(VmValue::Bool(heap.get_map(r).contains_key(k.as_str()))),
        ("keys", []) => {
            let mut keys: Vec<VmValue> = heap
                .get_map(r)
                .keys()
                .map(|k| VmValue::Str(k.clone()))
                .collect();
            keys.sort_by(vm_value_partial_cmp);
            Ok(VmValue::List(heap.alloc(HeapObject::List(keys))))
        }
        ("merge", [VmValue::Map(other_r)]) => {
            let mut new_map = heap.get_map(r).clone();
            for (k, v) in heap.get_map(*other_r).iter() {
                new_map.insert(k.clone(), v.clone());
            }
            Ok(VmValue::Map(heap.alloc(HeapObject::Map(new_map))))
        }
        ("set", [VmValue::Str(k), v]) => {
            let v = v.clone();
            heap.get_map_mut(r).insert(k.clone(), v.clone());
            Ok(v)
        }
        ("size", []) => Ok(VmValue::Int(heap.get_map(r).len() as i64)),
        ("to_s", []) => Ok(VmValue::Str(format_value_with_heap(heap, recv))),
        ("values", []) => {
            let mut pairs: Vec<(String, VmValue)> = heap
                .get_map(r)
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
            let vals: Vec<VmValue> = pairs.into_iter().map(|(_, v)| v).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(vals))))
        }
        ("delete", [_]) => Err(VmError::TypeError {
            message: "delete expects a String key".to_string(),
            line,
        }),
        ("get", [_]) => Err(VmError::TypeError {
            message: "get expects a String key".to_string(),
            line,
        }),
        ("has_key?", [_]) => Err(VmError::TypeError {
            message: "has_key? expects a String".to_string(),
            line,
        }),
        ("merge", [_]) => Err(VmError::TypeError {
            message: "merge expects a Map".to_string(),
            line,
        }),
        ("set", [_, _]) => Err(VmError::TypeError {
            message: "set expects a String key".to_string(),
            line,
        }),
        _ => Err(arg_error(name, args.len(), line)),
    }
}
