use crate::gc::{GcHeap, GcRef};
use crate::native::vm_value_partial_cmp;
use crate::vm::{define_native_method, format_value_with_heap, HeapObject, VmError, VmValue};

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

fn map_r(recv: &VmValue) -> GcRef {
    match recv {
        VmValue::Map(r) => *r,
        _ => unreachable!("Map native on non-Map"),
    }
}

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

macro_rules! map_native {
    ($fn:ident, $name:literal) => {
        pub fn $fn(
            heap: &mut GcHeap<HeapObject>,
            recv: &VmValue,
            args: &[VmValue],
            line: u32,
        ) -> Result<VmValue, VmError> {
            let r = map_r(recv);
            dispatch_map_method(heap, r, recv, $name, args, line)
        }
    };
}

map_native!(map_delete, "delete");
map_native!(map_empty_q, "empty?");
map_native!(map_get, "get");
map_native!(map_has_key_q, "has_key?");
map_native!(map_keys, "keys");
map_native!(map_merge, "merge");
map_native!(map_set, "set");
map_native!(map_size, "size");
map_native!(map_to_s, "to_s");
map_native!(map_values, "values");

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

