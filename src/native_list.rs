use crate::gc::{GcHeap, GcRef};
use crate::native::vm_value_partial_cmp;
use crate::vm::{
    define_native_method, format_value_with_heap, HeapObject, NativeArity, VmError, VmValue,
};

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("all?", 0),
    ("any?", 0),
    ("append", 1),
    ("concat", 1),
    ("empty?", 0),
    ("first", 0),
    ("flatten", 0),
    ("include?", 1),
    ("last", 0),
    ("max", 0),
    ("min", 0),
    ("pop", 0),
    ("prepend", 1),
    ("reverse", 0),
    ("size", 0),
    ("sort", 0),
    ("sum", 0),
    ("to_s", 0),
    ("uniq", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("List.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("List has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

fn list_r(recv: &VmValue) -> GcRef {
    match recv {
        VmValue::List(r) => *r,
        _ => unreachable!("List native on non-List"),
    }
}

fn flatten_value(heap: &GcHeap<HeapObject>, v: &VmValue) -> Vec<VmValue> {
    match v {
        VmValue::List(inner) => heap
            .get_list(*inner)
            .iter()
            .flat_map(|el| flatten_value(heap, el))
            .collect(),
        other => vec![other.clone()],
    }
}

pub fn dispatch_list_method(
    heap: &mut GcHeap<HeapObject>,
    r: GcRef,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match (name, args) {
        ("all?", []) => Err(VmError::TypeError {
            message: "all? requires a block".to_string(),
            line,
        }),
        ("any?", []) => Err(VmError::TypeError {
            message: "any? requires a block".to_string(),
            line,
        }),
        ("append", [x]) => {
            heap.get_list_mut(r).push(x.clone());
            Ok(recv.clone())
        }
        ("concat", [VmValue::List(other_r)]) => {
            let other_items: Vec<VmValue> = heap.get_list(*other_r).clone();
            heap.get_list_mut(r).extend(other_items);
            Ok(recv.clone())
        }
        ("empty?", []) => Ok(VmValue::Bool(heap.get_list(r).is_empty())),
        ("first", []) => Ok(heap.get_list(r).first().cloned().unwrap_or(VmValue::Nil)),
        ("flatten", []) => {
            let v: Vec<VmValue> = heap
                .get_list(r)
                .iter()
                .flat_map(|el| flatten_value(heap, el))
                .collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        ("include?", [item]) => Ok(VmValue::Bool(heap.get_list(r).contains(item))),
        ("join", []) => {
            let s = heap
                .get_list(r)
                .iter()
                .map(|v| format_value_with_heap(heap, v))
                .collect::<Vec<_>>()
                .join("");
            Ok(VmValue::Str(s))
        }
        ("join", [VmValue::Str(sep), ..]) => {
            let s = heap
                .get_list(r)
                .iter()
                .map(|v| format_value_with_heap(heap, v))
                .collect::<Vec<_>>()
                .join(sep.as_str());
            Ok(VmValue::Str(s))
        }
        ("last", []) => Ok(heap.get_list(r).last().cloned().unwrap_or(VmValue::Nil)),
        ("max", []) => {
            let v = heap.get_list(r);
            if v.is_empty() {
                return Ok(VmValue::Nil);
            }
            Ok(v
                .iter()
                .max_by(|a, b| vm_value_partial_cmp(a, b))
                .cloned()
                .unwrap())
        }
        ("min", []) => {
            let v = heap.get_list(r);
            if v.is_empty() {
                return Ok(VmValue::Nil);
            }
            Ok(v
                .iter()
                .min_by(|a, b| vm_value_partial_cmp(a, b))
                .cloned()
                .unwrap())
        }
        ("pop", []) => Ok(heap.get_list_mut(r).pop().unwrap_or(VmValue::Nil)),
        ("prepend", [x]) => {
            heap.get_list_mut(r).insert(0, x.clone());
            Ok(recv.clone())
        }
        ("reverse", []) => {
            let v: Vec<VmValue> = heap.get_list(r).iter().cloned().rev().collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        ("size", []) => Ok(VmValue::Int(heap.get_list(r).len() as i64)),
        ("sort", []) => {
            let mut v: Vec<VmValue> = heap.get_list(r).clone();
            v.sort_by(vm_value_partial_cmp);
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        ("sum", []) => {
            let items: Vec<VmValue> = heap.get_list(r).clone();
            let mut acc = VmValue::Int(0);
            for item in items.iter() {
                acc = match (&acc, item) {
                    (VmValue::Int(a), VmValue::Int(b)) => VmValue::Int(a + b),
                    (VmValue::Float(a), VmValue::Float(b)) => VmValue::Float(a + b),
                    (VmValue::Int(a), VmValue::Float(b)) => VmValue::Float(*a as f64 + b),
                    (VmValue::Float(a), VmValue::Int(b)) => VmValue::Float(a + *b as f64),
                    _ => {
                        return Err(VmError::TypeError {
                            message: "sum: non-numeric element".to_string(),
                            line,
                        })
                    }
                };
            }
            Ok(acc)
        }
        ("to_s", []) => Ok(VmValue::Str(format_value_with_heap(heap, recv))),
        ("uniq", []) => {
            let mut seen = Vec::new();
            for item in heap.get_list(r).iter() {
                if !seen.contains(item) {
                    seen.push(item.clone());
                }
            }
            Ok(VmValue::List(heap.alloc(HeapObject::List(seen))))
        }
        ("concat", [_]) => Err(VmError::TypeError {
            message: "concat expects a List".to_string(),
            line,
        }),
        ("join", _) => Err(VmError::TypeError {
            message: "join expects a String".to_string(),
            line,
        }),
        _ => Err(arg_error(name, args.len(), line)),
    }
}

macro_rules! list_native {
    ($fn:ident, $name:literal) => {
        pub fn $fn(
            heap: &mut GcHeap<HeapObject>,
            recv: &VmValue,
            args: &[VmValue],
            line: u32,
        ) -> Result<VmValue, VmError> {
            let r = list_r(recv);
            dispatch_list_method(heap, r, recv, $name, args, line)
        }
    };
}

list_native!(list_all_q, "all?");
list_native!(list_any_q, "any?");
list_native!(list_append, "append");
list_native!(list_concat, "concat");
list_native!(list_empty_q, "empty?");
list_native!(list_first, "first");
list_native!(list_flatten, "flatten");
list_native!(list_include_q, "include?");
list_native!(list_last, "last");
list_native!(list_max, "max");
list_native!(list_min, "min");
list_native!(list_pop, "pop");
list_native!(list_prepend, "prepend");
list_native!(list_reverse, "reverse");
list_native!(list_size, "size");
list_native!(list_sort, "sort");
list_native!(list_sum, "sum");
list_native!(list_to_s, "to_s");
list_native!(list_uniq, "uniq");

pub fn list_join(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    dispatch_list_method(heap, r, recv, "join", args, line)
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "all?", 0, list_all_q);
    define_native_method(heap, class_ref, "any?", 0, list_any_q);
    define_native_method(heap, class_ref, "append", 1, list_append);
    define_native_method(heap, class_ref, "concat", 1, list_concat);
    define_native_method(heap, class_ref, "empty?", 0, list_empty_q);
    define_native_method(heap, class_ref, "first", 0, list_first);
    define_native_method(heap, class_ref, "flatten", 0, list_flatten);
    define_native_method(heap, class_ref, "include?", 1, list_include_q);
    define_native_method(
        heap,
        class_ref,
        "join",
        NativeArity::at_least(0),
        list_join,
    );
    define_native_method(heap, class_ref, "last", 0, list_last);
    define_native_method(heap, class_ref, "max", 0, list_max);
    define_native_method(heap, class_ref, "min", 0, list_min);
    define_native_method(heap, class_ref, "pop", 0, list_pop);
    define_native_method(heap, class_ref, "prepend", 1, list_prepend);
    define_native_method(heap, class_ref, "reverse", 0, list_reverse);
    define_native_method(heap, class_ref, "size", 0, list_size);
    define_native_method(heap, class_ref, "sort", 0, list_sort);
    define_native_method(heap, class_ref, "sum", 0, list_sum);
    define_native_method(heap, class_ref, "to_s", 0, list_to_s);
    define_native_method(heap, class_ref, "uniq", 0, list_uniq);
}

