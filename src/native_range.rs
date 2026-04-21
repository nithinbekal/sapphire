use crate::gc::GcHeap;
use crate::vm::{HeapObject, VmError, VmValue};

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
