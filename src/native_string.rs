use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, NativeArity, VmError, VmValue};

fn str_recv(recv: &VmValue) -> &str {
    match recv {
        VmValue::Str(s) => s.as_str(),
        _ => unreachable!("String native on non-Str"),
    }
}

pub fn string_bytes(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    let bytes: Vec<VmValue> = s.bytes().map(|b| VmValue::Int(b as i64)).collect();
    Ok(VmValue::List(heap.alloc(HeapObject::List(bytes))))
}

pub fn string_chars(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    let chars: Vec<VmValue> = s.chars().map(|c| VmValue::Str(c.to_string())).collect();
    Ok(VmValue::List(heap.alloc(HeapObject::List(chars))))
}

pub fn string_chomp(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    Ok(VmValue::Str(s.trim_end_matches('\n').to_string()))
}

pub fn string_downcase(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(str_recv(recv).to_lowercase()))
}

pub fn string_empty_q(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Bool(str_recv(recv).is_empty()))
}

pub fn string_ends_with_q(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    match args {
        [VmValue::Str(pat)] => Ok(VmValue::Bool(s.ends_with(pat.as_str()))),
        [_] => Err(VmError::TypeError {
            message: "ends_with? expects a String".to_string(),
            line,
        }),
        _ => unreachable!("String#ends_with?: expected 1 argument"),
    }
}

pub fn string_include_q(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    match args {
        [VmValue::Str(pat)] => Ok(VmValue::Bool(s.contains(pat.as_str()))),
        [_] => Err(VmError::TypeError {
            message: "include? expects a String".to_string(),
            line,
        }),
        _ => unreachable!("String#include?: expected 1 argument"),
    }
}

pub fn string_lines(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    let lines: Vec<VmValue> = s.lines().map(|l| VmValue::Str(l.to_string())).collect();
    Ok(VmValue::List(heap.alloc(HeapObject::List(lines))))
}

pub fn string_replace(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    match args {
        [VmValue::Str(from), VmValue::Str(to)] => Ok(VmValue::Str(
            s.replacen(from.as_str(), to.as_str(), 1),
        )),
        [_, _] => Err(VmError::TypeError {
            message: "replace expects two Strings".to_string(),
            line,
        }),
        _ => unreachable!("String#replace: expected 2 arguments"),
    }
}

pub fn string_replace_all(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    match args {
        [VmValue::Str(from), VmValue::Str(to)] => {
            Ok(VmValue::Str(s.replace(from.as_str(), to.as_str())))
        }
        [_, _] => Err(VmError::TypeError {
            message: "replace_all expects two Strings".to_string(),
            line,
        }),
        _ => unreachable!("String#replace_all: expected 2 arguments"),
    }
}

pub fn string_reverse(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(str_recv(recv).chars().rev().collect()))
}

pub fn string_size(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Int(str_recv(recv).chars().count() as i64))
}

pub fn string_slice(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    match args {
        [VmValue::Int(start), VmValue::Int(len)] => {
            let chars: Vec<char> = s.chars().collect();
            let n = chars.len() as i64;
            let start = if *start < 0 {
                (n + start).max(0) as usize
            } else {
                *start as usize
            };
            let len = *len as usize;
            let end = (start + len).min(chars.len());
            Ok(VmValue::Str(chars[start..end].iter().collect()))
        }
        [_, _] => Err(VmError::TypeError {
            message: "slice expects (Int, Int)".to_string(),
            line,
        }),
        _ => unreachable!("String#slice: expected 2 arguments"),
    }
}

pub fn string_split(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    match args {
        [] => {
            let parts: Vec<VmValue> = s
                .split_whitespace()
                .map(|p| VmValue::Str(p.to_string()))
                .collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(parts))))
        }
        [VmValue::Str(sep)] => {
            let parts: Vec<VmValue> = s
                .split(sep.as_str())
                .map(|p| VmValue::Str(p.to_string()))
                .collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(parts))))
        }
        _ => Err(VmError::TypeError {
            message: "split expects a String delimiter".to_string(),
            line,
        }),
    }
}

pub fn string_starts_with_q(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let s = str_recv(recv);
    match args {
        [VmValue::Str(pat)] => Ok(VmValue::Bool(s.starts_with(pat.as_str()))),
        [_] => Err(VmError::TypeError {
            message: "starts_with? expects a String".to_string(),
            line,
        }),
        _ => unreachable!("String#starts_with?: expected 1 argument"),
    }
}

pub fn string_to_f(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Float(str_recv(recv).trim().parse::<f64>().unwrap_or(0.0)))
}

pub fn string_to_i(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Int(str_recv(recv).trim().parse::<i64>().unwrap_or(0)))
}

pub fn string_to_s(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(str_recv(recv).to_string()))
}

pub fn string_trim(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(str_recv(recv).trim().to_string()))
}

pub fn string_upcase(
    _heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    Ok(VmValue::Str(str_recv(recv).to_uppercase()))
}

pub fn register_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_method(heap, class_ref, "bytes", 0, string_bytes);
    define_native_method(heap, class_ref, "chars", 0, string_chars);
    define_native_method(heap, class_ref, "chomp", 0, string_chomp);
    define_native_method(heap, class_ref, "downcase", 0, string_downcase);
    define_native_method(heap, class_ref, "empty?", 0, string_empty_q);
    define_native_method(heap, class_ref, "ends_with?", 1, string_ends_with_q);
    define_native_method(heap, class_ref, "include?", 1, string_include_q);
    define_native_method(heap, class_ref, "lines", 0, string_lines);
    define_native_method(heap, class_ref, "replace", 2, string_replace);
    define_native_method(heap, class_ref, "replace_all", 2, string_replace_all);
    define_native_method(heap, class_ref, "reverse", 0, string_reverse);
    define_native_method(heap, class_ref, "size", 0, string_size);
    define_native_method(heap, class_ref, "slice", 2, string_slice);
    define_native_method(heap, class_ref, "starts_with?", 1, string_starts_with_q);
    define_native_method(heap, class_ref, "split", NativeArity { min: 0, max: 1 }, string_split);
    define_native_method(heap, class_ref, "to_f", 0, string_to_f);
    define_native_method(heap, class_ref, "to_i", 0, string_to_i);
    define_native_method(heap, class_ref, "to_s", 0, string_to_s);
    define_native_method(heap, class_ref, "trim", 0, string_trim);
    define_native_method(heap, class_ref, "upcase", 0, string_upcase);
}
