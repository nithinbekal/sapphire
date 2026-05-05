use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_method, HeapObject, NativeArity, VmError, VmValue};

fn str_recv(recv: &VmValue) -> &str {
    match recv {
        VmValue::Str(s) => s.as_str(),
        _ => unreachable!("String native on non-Str"),
    }
}

pub fn dispatch_str_method(
    heap: &mut GcHeap<HeapObject>,
    s: &str,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match (name, args) {
        ("bytes", []) => {
            let bytes: Vec<VmValue> = s.bytes().map(|b| VmValue::Int(b as i64)).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(bytes))))
        }
        ("chars", []) => {
            let chars: Vec<VmValue> = s.chars().map(|c| VmValue::Str(c.to_string())).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(chars))))
        }
        ("chomp", []) => Ok(VmValue::Str(s.trim_end_matches('\n').to_string())),
        ("downcase", []) => Ok(VmValue::Str(s.to_lowercase())),
        ("empty?", []) => Ok(VmValue::Bool(s.is_empty())),
        ("ends_with?", [VmValue::Str(pat)]) => Ok(VmValue::Bool(s.ends_with(pat.as_str()))),
        ("include?", [VmValue::Str(pat)]) => Ok(VmValue::Bool(s.contains(pat.as_str()))),
        ("lines", []) => {
            let lines: Vec<VmValue> = s.lines().map(|l| VmValue::Str(l.to_string())).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(lines))))
        }
        ("replace", [VmValue::Str(from), VmValue::Str(to)]) => Ok(VmValue::Str(
            s.replacen(from.as_str(), to.as_str(), 1),
        )),
        ("replace_all", [VmValue::Str(from), VmValue::Str(to)]) => Ok(VmValue::Str(
            s.replace(from.as_str(), to.as_str()),
        )),
        ("reverse", []) => Ok(VmValue::Str(s.chars().rev().collect())),
        ("size", []) => Ok(VmValue::Int(s.chars().count() as i64)),
        ("slice", [VmValue::Int(start), VmValue::Int(len)]) => {
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
        ("split", []) => {
            let parts: Vec<VmValue> = s
                .split_whitespace()
                .map(|p| VmValue::Str(p.to_string()))
                .collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(parts))))
        }
        ("split", [VmValue::Str(sep)]) => {
            let parts: Vec<VmValue> = s
                .split(sep.as_str())
                .map(|p| VmValue::Str(p.to_string()))
                .collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(parts))))
        }
        ("starts_with?", [VmValue::Str(pat)]) => Ok(VmValue::Bool(s.starts_with(pat.as_str()))),
        ("to_f", []) => Ok(VmValue::Float(s.trim().parse::<f64>().unwrap_or(0.0))),
        ("to_i", []) => Ok(VmValue::Int(s.trim().parse::<i64>().unwrap_or(0))),
        ("to_s", []) => Ok(VmValue::Str(s.to_string())),
        ("trim", []) => Ok(VmValue::Str(s.trim().to_string())),
        ("upcase", []) => Ok(VmValue::Str(s.to_uppercase())),
        (m @ ("ends_with?" | "include?" | "starts_with?"), [_]) => Err(VmError::TypeError {
            message: format!("{m} expects a String"),
            line,
        }),
        ("replace", [_, _]) => Err(VmError::TypeError {
            message: "replace expects two Strings".to_string(),
            line,
        }),
        ("replace_all", [_, _]) => Err(VmError::TypeError {
            message: "replace_all expects two Strings".to_string(),
            line,
        }),
        ("slice", [_, _]) => Err(VmError::TypeError {
            message: "slice expects (Int, Int)".to_string(),
            line,
        }),
        ("split", _) => Err(VmError::TypeError {
            message: "split expects a String delimiter".to_string(),
            line,
        }),
        _ => unreachable!("dispatch_str_method({name:?}, {} args)", args.len()),
    }
}

macro_rules! str_native {
    ($fn:ident, $name:literal) => {
        pub fn $fn(
            heap: &mut GcHeap<HeapObject>,
            recv: &VmValue,
            args: &[VmValue],
            line: u32,
        ) -> Result<VmValue, VmError> {
            dispatch_str_method(heap, str_recv(recv), $name, args, line)
        }
    };
}

str_native!(string_bytes, "bytes");
str_native!(string_chars, "chars");
str_native!(string_chomp, "chomp");
str_native!(string_downcase, "downcase");
str_native!(string_empty_q, "empty?");
str_native!(string_ends_with_q, "ends_with?");
str_native!(string_include_q, "include?");
str_native!(string_lines, "lines");
str_native!(string_replace, "replace");
str_native!(string_replace_all, "replace_all");
str_native!(string_reverse, "reverse");
str_native!(string_size, "size");
str_native!(string_slice, "slice");
str_native!(string_starts_with_q, "starts_with?");
str_native!(string_to_f, "to_f");
str_native!(string_to_i, "to_i");
str_native!(string_to_s, "to_s");
str_native!(string_trim, "trim");
str_native!(string_upcase, "upcase");

pub fn string_split(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    dispatch_str_method(heap, str_recv(recv), "split", args, line)
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
    define_native_method(
        heap,
        class_ref,
        "split",
        NativeArity { min: 0, max: 1 },
        string_split,
    );
    define_native_method(heap, class_ref, "to_f", 0, string_to_f);
    define_native_method(heap, class_ref, "to_i", 0, string_to_i);
    define_native_method(heap, class_ref, "to_s", 0, string_to_s);
    define_native_method(heap, class_ref, "trim", 0, string_trim);
    define_native_method(heap, class_ref, "upcase", 0, string_upcase);
}
