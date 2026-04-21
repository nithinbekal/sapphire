use crate::gc::GcHeap;
use crate::vm::{HeapObject, VmError, VmValue};

pub fn dispatch_str_method(
    heap: &mut GcHeap<HeapObject>,
    s: &str,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match name {
        "size" if args.is_empty() => Ok(VmValue::Int(s.chars().count() as i64)),
        "upcase" if args.is_empty() => Ok(VmValue::Str(s.to_uppercase())),
        "downcase" if args.is_empty() => Ok(VmValue::Str(s.to_lowercase())),
        "reverse" if args.is_empty() => Ok(VmValue::Str(s.chars().rev().collect())),
        "trim" if args.is_empty() => Ok(VmValue::Str(s.trim().to_string())),
        "chomp" if args.is_empty() => Ok(VmValue::Str(s.trim_end_matches('\n').to_string())),
        "to_i" if args.is_empty() => Ok(VmValue::Int(s.trim().parse::<i64>().unwrap_or(0))),
        "to_f" if args.is_empty() => Ok(VmValue::Float(s.trim().parse::<f64>().unwrap_or(0.0))),
        "to_s" if args.is_empty() => Ok(VmValue::Str(s.to_string())),
        "empty?" if args.is_empty() => Ok(VmValue::Bool(s.is_empty())),
        "chars" if args.is_empty() => {
            let chars: Vec<VmValue> = s.chars().map(|c| VmValue::Str(c.to_string())).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(chars))))
        }
        "bytes" if args.is_empty() => {
            let bytes: Vec<VmValue> = s.bytes().map(|b| VmValue::Int(b as i64)).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(bytes))))
        }
        "lines" if args.is_empty() => {
            let lines: Vec<VmValue> = s.lines().map(|l| VmValue::Str(l.to_string())).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(lines))))
        }
        "include?" if args.len() == 1 => match &args[0] {
            VmValue::Str(pat) => Ok(VmValue::Bool(s.contains(pat.as_str()))),
            _ => Err(type_err("include? expects a String")),
        },
        "starts_with?" if args.len() == 1 => match &args[0] {
            VmValue::Str(pat) => Ok(VmValue::Bool(s.starts_with(pat.as_str()))),
            _ => Err(type_err("starts_with? expects a String")),
        },
        "ends_with?" if args.len() == 1 => match &args[0] {
            VmValue::Str(pat) => Ok(VmValue::Bool(s.ends_with(pat.as_str()))),
            _ => Err(type_err("ends_with? expects a String")),
        },
        "split" => match args {
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
            _ => Err(type_err("split expects a String delimiter")),
        },
        "replace" if args.len() == 2 => match (&args[0], &args[1]) {
            (VmValue::Str(from), VmValue::Str(to)) => {
                Ok(VmValue::Str(s.replacen(from.as_str(), to.as_str(), 1)))
            }
            _ => Err(type_err("replace expects two Strings")),
        },
        "replace_all" if args.len() == 2 => match (&args[0], &args[1]) {
            (VmValue::Str(from), VmValue::Str(to)) => {
                Ok(VmValue::Str(s.replace(from.as_str(), to.as_str())))
            }
            _ => Err(type_err("replace_all expects two Strings")),
        },
        "slice" if args.len() == 2 => match (&args[0], &args[1]) {
            (VmValue::Int(start), VmValue::Int(len)) => {
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
            _ => Err(type_err("slice expects (Int, Int)")),
        },
        _ => Err(type_err(&format!("String has no method '{}'", name))),
    }
}
