use crate::gc::GcHeap;
use crate::vm::{HeapObject, VmError, VmValue};

pub const NATIVE_METHOD_NAMES: &[&str] = &[
    "bytes",
    "chars",
    "chomp",
    "downcase",
    "empty?",
    "ends_with?",
    "include?",
    "lines",
    "replace",
    "replace_all",
    "reverse",
    "size",
    "slice",
    "split",
    "starts_with?",
    "to_f",
    "to_i",
    "to_s",
    "trim",
    "upcase",
];

const METHOD_ARITIES: &[(&str, usize)] = &[
    ("bytes", 0),
    ("chars", 0),
    ("chomp", 0),
    ("downcase", 0),
    ("empty?", 0),
    ("ends_with?", 1),
    ("include?", 1),
    ("lines", 0),
    ("replace", 2),
    ("replace_all", 2),
    ("reverse", 0),
    ("size", 0),
    ("slice", 2),
    ("starts_with?", 1),
    ("to_f", 0),
    ("to_i", 0),
    ("to_s", 0),
    ("trim", 0),
    ("upcase", 0),
];

fn arg_error(name: &str, argc: usize, line: u32) -> VmError {
    let msg = METHOD_ARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arity)| format!("String.{name} expects {arity} argument(s), got {argc}"))
        .unwrap_or_else(|| format!("String has no method '{name}'"));
    VmError::TypeError { message: msg, line }
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
        _ => Err(arg_error(name, args.len(), line)),
    }
}
