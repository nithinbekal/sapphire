use crate::gc::{GcHeap, GcRef};
use crate::native::vm_value_partial_cmp;
use crate::vm::{
    define_native_method, format_value_with_heap, HeapObject, NativeArity, VmError, VmValue,
};

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

pub fn list_all_q(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    _args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    Err(VmError::TypeError {
        message: "all? requires a block".to_string(),
        line,
    })
}

pub fn list_any_q(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    _args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    Err(VmError::TypeError {
        message: "any? requires a block".to_string(),
        line,
    })
}

pub fn list_append(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    heap.get_list_mut(r).push(args[0].clone());
    Ok(recv.clone())
}

pub fn list_concat(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    match &args[0] {
        VmValue::List(other_r) => {
            let other_items: Vec<VmValue> = heap.get_list(*other_r).clone();
            heap.get_list_mut(r).extend(other_items);
            Ok(recv.clone())
        }
        _ => Err(VmError::TypeError {
            message: "concat expects a List".to_string(),
            line,
        }),
    }
}

pub fn list_empty_q(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    Ok(VmValue::Bool(heap.get_list(r).is_empty()))
}

pub fn list_first(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    Ok(heap.get_list(r).first().cloned().unwrap_or(VmValue::Nil))
}

pub fn list_flatten(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    let v: Vec<VmValue> = heap
        .get_list(r)
        .iter()
        .flat_map(|el| flatten_value(heap, el))
        .collect();
    Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
}

pub fn list_include_q(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    Ok(VmValue::Bool(heap.get_list(r).contains(&args[0])))
}

pub fn list_join(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    let sep = match args {
        [] => "",
        [VmValue::Str(s)] => s.as_str(),
        _ => {
            return Err(VmError::TypeError {
                message: "join expects a String".to_string(),
                line,
            })
        }
    };
    let s = heap
        .get_list(r)
        .iter()
        .map(|v| format_value_with_heap(heap, v))
        .collect::<Vec<_>>()
        .join(sep);
    Ok(VmValue::Str(s))
}

pub fn list_last(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    Ok(heap.get_list(r).last().cloned().unwrap_or(VmValue::Nil))
}

pub fn list_max(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    let v = heap.get_list(r);
    if v.is_empty() {
        return Ok(VmValue::Nil);
    }
    Ok(v.iter().max_by(|a, b| vm_value_partial_cmp(a, b)).cloned().unwrap())
}

pub fn list_min(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    let v = heap.get_list(r);
    if v.is_empty() {
        return Ok(VmValue::Nil);
    }
    Ok(v.iter().min_by(|a, b| vm_value_partial_cmp(a, b)).cloned().unwrap())
}

pub fn list_pop(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    Ok(heap.get_list_mut(r).pop().unwrap_or(VmValue::Nil))
}

pub fn list_prepend(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    heap.get_list_mut(r).insert(0, args[0].clone());
    Ok(recv.clone())
}

pub fn list_reverse(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    let v: Vec<VmValue> = heap.get_list(r).iter().cloned().rev().collect();
    Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
}

pub fn list_size(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    Ok(VmValue::Int(heap.get_list(r).len() as i64))
}

pub fn list_sort(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    let mut v: Vec<VmValue> = heap.get_list(r).clone();
    v.sort_by(vm_value_partial_cmp);
    Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
}

pub fn list_sum(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
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

pub fn list_to_s(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(format_value_with_heap(heap, recv)))
}

pub fn list_uniq(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let r = list_r(recv);
    let mut seen = Vec::new();
    for item in heap.get_list(r).iter() {
        if !seen.contains(item) {
            seen.push(item.clone());
        }
    }
    Ok(VmValue::List(heap.alloc(HeapObject::List(seen))))
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
