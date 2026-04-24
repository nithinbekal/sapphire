use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, VmError, VmValue};

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("first", 0),
    ("include?", 1),
    ("last", 0),
    ("max", 0),
    ("min", 0),
    ("size", 0),
    ("to_a", 0),
    ("to_s", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("Range.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("Range has no method '{name}'"));
    VmError::TypeError { message: msg, line }
}

fn range_recv(recv: &VmValue) -> (i64, i64) {
    match recv {
        VmValue::Range { from, to } => (*from, *to),
        _ => unreachable!("Range native on non-Range"),
    }
}

pub fn dispatch_range_method(
    heap: &mut GcHeap<HeapObject>,
    from: i64,
    to: i64,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match (name, args) {
        ("first", []) => Ok(VmValue::Int(from)),
        ("include?", [VmValue::Int(n)]) => Ok(VmValue::Bool(n >= &from && n < &to)),
        ("last", []) => Ok(VmValue::Int(to - 1)),
        ("max", []) => Ok(VmValue::Int(to - 1)),
        ("min", []) => Ok(VmValue::Int(from)),
        ("size", []) => Ok(VmValue::Int((to - from).max(0))),
        ("to_a", []) => {
            let v: Vec<VmValue> = (from..to).map(VmValue::Int).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        ("to_s", []) => Ok(VmValue::Str(format!("{}", recv))),
        ("include?", [_]) => Err(VmError::TypeError {
            message: "include? expects an Int".to_string(),
            line,
        }),
        _ => Err(arg_error(name, args.len(), line)),
    }
}

macro_rules! range_native {
    ($fn:ident, $name:literal) => {
        pub fn $fn(
            heap: &mut GcHeap<HeapObject>,
            recv: &VmValue,
            args: &[VmValue],
            line: u32,
        ) -> Result<VmValue, VmError> {
            let (from, to) = range_recv(recv);
            dispatch_range_method(heap, from, to, recv, $name, args, line)
        }
    };
}

range_native!(range_first, "first");
range_native!(range_include_q, "include?");
range_native!(range_last, "last");
range_native!(range_max, "max");
range_native!(range_min, "min");
range_native!(range_size, "size");
range_native!(range_to_a, "to_a");
range_native!(range_to_s, "to_s");

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "first", 0, range_first);
    define_native_method(heap, class_ref, "include?", 1, range_include_q);
    define_native_method(heap, class_ref, "last", 0, range_last);
    define_native_method(heap, class_ref, "max", 0, range_max);
    define_native_method(heap, class_ref, "min", 0, range_min);
    define_native_method(heap, class_ref, "size", 0, range_size);
    define_native_method(heap, class_ref, "to_a", 0, range_to_a);
    define_native_method(heap, class_ref, "to_s", 0, range_to_s);
}
