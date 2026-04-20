use crate::gc::{GcHeap, GcRef};
use crate::vm::{format_value_with_heap, HeapObject, VmError, VmValue};
use std::cmp::Ordering;

// ── Public utilities used throughout the VM ───────────────────────────────────

pub fn is_falsy(v: &VmValue) -> bool {
    matches!(v, VmValue::Nil | VmValue::Bool(false))
}

/// Return the stdlib class name for a primitive value, used to look up
/// compiled stdlib methods in the class registry.
pub fn primitive_class_name(val: &VmValue) -> Option<&'static str> {
    match val {
        VmValue::Int(_) => Some("Int"),
        VmValue::Float(_) => Some("Float"),
        VmValue::Str(_) => Some("String"),
        VmValue::Bool(_) => Some("Bool"),
        VmValue::Nil => Some("Nil"),
        VmValue::List(_) => Some("List"),
        VmValue::Map(_) => Some("Map"),
        VmValue::Set(_) => Some("Set"),
        _ => None,
    }
}

/// Return the type name of a value for use in runtime type-checking error messages.
pub fn value_type_name(val: &VmValue) -> &str {
    match val {
        VmValue::Int(_) => "Int",
        VmValue::Float(_) => "Float",
        VmValue::Str(_) => "String",
        VmValue::Bool(_) => "Bool",
        VmValue::Nil => "Nil",
        VmValue::List(_) => "List",
        VmValue::Map(_) => "Map",
        VmValue::Set(_) => "Set",
        VmValue::Range { .. } => "Range",
        VmValue::Instance { class_name, .. } => class_name.as_str(),
        VmValue::Class { name, .. } => name.as_str(),
        VmValue::Function(_) => "Function",
        VmValue::Closure { .. } => "Function",
    }
}

/// Simple comparison for sorting — numbers compare numerically, strings lexicographically.
pub fn vm_value_partial_cmp(a: &VmValue, b: &VmValue) -> Ordering {
    match (a, b) {
        (VmValue::Int(x), VmValue::Int(y)) => x.cmp(y),
        (VmValue::Float(x), VmValue::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (VmValue::Int(x), VmValue::Float(y)) => {
            (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        (VmValue::Float(x), VmValue::Int(y)) => {
            x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal)
        }
        (VmValue::Str(x), VmValue::Str(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

pub fn numeric_binop(
    a: &VmValue,
    b: &VmValue,
    line: u32,
    verb: &str,
    int_op: impl Fn(i64, i64) -> i64,
    float_op: impl Fn(f64, f64) -> f64,
) -> Result<VmValue, VmError> {
    match (a, b) {
        (VmValue::Int(x), VmValue::Int(y)) => Ok(VmValue::Int(int_op(*x, *y))),
        (VmValue::Float(x), VmValue::Float(y)) => Ok(VmValue::Float(float_op(*x, *y))),
        (VmValue::Int(x), VmValue::Float(y)) => Ok(VmValue::Float(float_op(*x as f64, *y))),
        (VmValue::Float(x), VmValue::Int(y)) => Ok(VmValue::Float(float_op(*x, *y as f64))),
        _ => Err(VmError::TypeError {
            message: format!("cannot {} {} and {}", verb, a, b),
            line,
        }),
    }
}

pub fn numeric_cmp(
    a: &VmValue,
    b: &VmValue,
    line: u32,
    op: impl Fn(f64, f64) -> bool,
) -> Result<bool, VmError> {
    let x = to_float(a).ok_or_else(|| VmError::TypeError {
        message: format!("cannot compare {} and {}", a, b),
        line,
    })?;
    let y = to_float(b).ok_or_else(|| VmError::TypeError {
        message: format!("cannot compare {} and {}", a, b),
        line,
    })?;
    Ok(op(x, y))
}

fn to_float(v: &VmValue) -> Option<f64> {
    match v {
        VmValue::Int(n) => Some(*n as f64),
        VmValue::Float(n) => Some(*n),
        _ => None,
    }
}

// ── Native method dispatch ────────────────────────────────────────────────────

/// Dispatch a native (non-block) method call on a built-in type.
pub fn dispatch_native_method(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match recv {
        VmValue::Int(n) => dispatch_int_method(*n, name, args, line),
        VmValue::Float(n) => dispatch_float_method(*n, name, args, line),
        VmValue::Str(s) => dispatch_str_method(heap, s, name, args, line),
        VmValue::Bool(b) => dispatch_bool_method(*b, name, args, line),
        VmValue::Nil => dispatch_nil_method(name, args, line),
        VmValue::List(r) => dispatch_list_method(heap, *r, recv, name, args, line),
        VmValue::Map(r) => dispatch_map_method(heap, *r, recv, name, args, line),
        VmValue::Set(r) => crate::native_set::dispatch_set_method(heap, *r, recv, name, args, line),
        VmValue::Range { from, to } => {
            dispatch_range_method(heap, *from, *to, recv, name, args, line)
        }
        other => Err(VmError::TypeError {
            message: format!("'{}' has no method '{}'", other, name),
            line,
        }),
    }
}

/// Like `dispatch_native_method` but returns `None` when no native handler
/// exists for this method, allowing callers to try the class registry next.
/// Any real type error (wrong arg count, wrong type, etc.) is still `Some(Err)`.
pub fn try_native_method(
    heap: &mut GcHeap<HeapObject>,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Option<Result<VmValue, VmError>> {
    match dispatch_native_method(heap, recv, name, args, line) {
        Err(VmError::TypeError { ref message, .. }) if message.contains("has no method") => None,
        result => Some(result),
    }
}

// ── Per-type dispatch ─────────────────────────────────────────────────────────

fn dispatch_int_method(n: i64, name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match (name, args) {
        ("to_s", []) => Ok(VmValue::Str(n.to_string())),
        ("to_f", []) => Ok(VmValue::Float(n as f64)),
        ("pow", [VmValue::Int(e)]) if *e >= 0 => Ok(VmValue::Int(n.pow(*e as u32))),
        _ => Err(type_err(&format!("Int has no method '{}'", name))),
    }
}

fn dispatch_float_method(n: f64, name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match (name, args) {
        ("to_s", []) => Ok(VmValue::Str(if n.fract() == 0.0 {
            format!("{}.0", n as i64)
        } else {
            format!("{}", n)
        })),
        ("to_i", []) => Ok(VmValue::Int(n as i64)),
        ("round", []) => Ok(VmValue::Int(n.round() as i64)),
        ("floor", []) => Ok(VmValue::Int(n.floor() as i64)),
        ("ceil", []) => Ok(VmValue::Int(n.ceil() as i64)),
        ("sqrt", []) => Ok(VmValue::Float(n.sqrt())),
        ("nan?", []) => Ok(VmValue::Bool(n.is_nan())),
        ("infinite?", []) => Ok(VmValue::Bool(n.is_infinite())),
        _ => Err(type_err(&format!("Float has no method '{}'", name))),
    }
}

fn dispatch_str_method(
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
        "strip" | "trim" if args.is_empty() => Ok(VmValue::Str(s.trim().to_string())),
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

fn dispatch_bool_method(b: bool, name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match (name, args) {
        ("to_s", []) => Ok(VmValue::Str(b.to_string())),
        ("nil?", []) => Ok(VmValue::Bool(false)),
        _ => Err(type_err(&format!("Bool has no method '{}'", name))),
    }
}

fn dispatch_nil_method(name: &str, args: &[VmValue], line: u32) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match (name, args) {
        ("to_s", []) => Ok(VmValue::Str(String::new())),
        ("nil?", []) => Ok(VmValue::Bool(true)),
        ("inspect", []) => Ok(VmValue::Str("nil".to_string())),
        _ => Err(type_err(&format!("Nil has no method '{}'", name))),
    }
}

fn dispatch_list_method(
    heap: &mut GcHeap<HeapObject>,
    r: GcRef,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match name {
        "size" if args.is_empty() => Ok(VmValue::Int(heap.get_list(r).len() as i64)),
        "empty?" if args.is_empty() => Ok(VmValue::Bool(heap.get_list(r).is_empty())),
        "first" if args.is_empty() => Ok(heap.get_list(r).first().cloned().unwrap_or(VmValue::Nil)),
        "last" if args.is_empty() => Ok(heap.get_list(r).last().cloned().unwrap_or(VmValue::Nil)),
        "pop" if args.is_empty() => Ok(heap.get_list_mut(r).pop().unwrap_or(VmValue::Nil)),
        "reverse" if args.is_empty() => {
            let v: Vec<VmValue> = heap.get_list(r).iter().cloned().rev().collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        "sort" if args.is_empty() => {
            let mut v: Vec<VmValue> = heap.get_list(r).clone();
            v.sort_by(vm_value_partial_cmp);
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        "include?" if args.len() == 1 => Ok(VmValue::Bool(heap.get_list(r).contains(&args[0]))),
        "push" | "append" if args.len() == 1 => {
            heap.get_list_mut(r).push(args[0].clone());
            Ok(recv.clone())
        }
        "unshift" | "prepend" if args.len() == 1 => {
            heap.get_list_mut(r).insert(0, args[0].clone());
            Ok(recv.clone())
        }
        "concat" if args.len() == 1 => match &args[0] {
            VmValue::List(other_r) => {
                let other_items: Vec<VmValue> = heap.get_list(*other_r).clone();
                heap.get_list_mut(r).extend(other_items);
                Ok(recv.clone())
            }
            _ => Err(type_err("concat expects a List")),
        },
        "join" => {
            let sep = match args.first() {
                Some(VmValue::Str(s)) => s.clone(),
                None => String::new(),
                _ => return Err(type_err("join expects a String")),
            };
            let s = heap
                .get_list(r)
                .iter()
                .map(|v| format_value_with_heap(heap, v))
                .collect::<Vec<_>>()
                .join(&sep);
            Ok(VmValue::Str(s))
        }
        "flatten" if args.is_empty() => {
            fn flatten_list(heap: &GcHeap<HeapObject>, v: &VmValue) -> Vec<VmValue> {
                match v {
                    VmValue::List(inner) => heap
                        .get_list(*inner)
                        .iter()
                        .flat_map(|el| flatten_list(heap, el))
                        .collect(),
                    other => vec![other.clone()],
                }
            }
            let v: Vec<VmValue> = heap
                .get_list(r)
                .iter()
                .flat_map(|el| flatten_list(heap, el))
                .collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        "uniq" if args.is_empty() => {
            let mut seen = Vec::new();
            for item in heap.get_list(r).iter() {
                if !seen.contains(item) {
                    seen.push(item.clone());
                }
            }
            Ok(VmValue::List(heap.alloc(HeapObject::List(seen))))
        }
        "min" if args.is_empty() => {
            let v = heap.get_list(r);
            if v.is_empty() {
                return Ok(VmValue::Nil);
            }
            Ok(v.iter().min_by(|a, b| vm_value_partial_cmp(a, b)).cloned().unwrap())
        }
        "max" if args.is_empty() => {
            let v = heap.get_list(r);
            if v.is_empty() {
                return Ok(VmValue::Nil);
            }
            Ok(v.iter().max_by(|a, b| vm_value_partial_cmp(a, b)).cloned().unwrap())
        }
        "sum" if args.is_empty() => {
            let items: Vec<VmValue> = heap.get_list(r).clone();
            let mut acc = VmValue::Int(0);
            for item in items.iter() {
                acc = match (&acc, item) {
                    (VmValue::Int(a), VmValue::Int(b)) => VmValue::Int(a + b),
                    (VmValue::Float(a), VmValue::Float(b)) => VmValue::Float(a + b),
                    (VmValue::Int(a), VmValue::Float(b)) => VmValue::Float(*a as f64 + b),
                    (VmValue::Float(a), VmValue::Int(b)) => VmValue::Float(a + *b as f64),
                    _ => return Err(type_err("sum: non-numeric element")),
                };
            }
            Ok(acc)
        }
        "any?" if args.is_empty() => Err(type_err("any? requires a block")),
        "all?" if args.is_empty() => Err(type_err("all? requires a block")),
        "to_s" if args.is_empty() => Ok(VmValue::Str(format_value_with_heap(heap, recv))),
        _ => Err(type_err(&format!("List has no method '{}'", name))),
    }
}

fn dispatch_map_method(
    heap: &mut GcHeap<HeapObject>,
    r: GcRef,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match name {
        "size" if args.is_empty() => Ok(VmValue::Int(heap.get_map(r).len() as i64)),
        "empty?" if args.is_empty() => Ok(VmValue::Bool(heap.get_map(r).is_empty())),
        "keys" if args.is_empty() => {
            let mut keys: Vec<VmValue> = heap
                .get_map(r)
                .keys()
                .map(|k| VmValue::Str(k.clone()))
                .collect();
            keys.sort_by(vm_value_partial_cmp);
            Ok(VmValue::List(heap.alloc(HeapObject::List(keys))))
        }
        "values" if args.is_empty() => {
            let mut pairs: Vec<(String, VmValue)> = heap
                .get_map(r)
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
            let vals: Vec<VmValue> = pairs.into_iter().map(|(_, v)| v).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(vals))))
        }
        "has_key?" if args.len() == 1 => match &args[0] {
            VmValue::Str(k) => Ok(VmValue::Bool(heap.get_map(r).contains_key(k.as_str()))),
            _ => Err(type_err("has_key? expects a String")),
        },
        "get" if args.len() == 1 => match &args[0] {
            VmValue::Str(k) => Ok(heap.get_map(r).get(k.as_str()).cloned().unwrap_or(VmValue::Nil)),
            _ => Err(type_err("get expects a String key")),
        },
        "set" if args.len() == 2 => match &args[0] {
            VmValue::Str(k) => {
                let (k, v) = (k.clone(), args[1].clone());
                heap.get_map_mut(r).insert(k, v);
                Ok(args[1].clone())
            }
            _ => Err(type_err("set expects a String key")),
        },
        "delete" if args.len() == 1 => match &args[0] {
            VmValue::Str(k) => Ok(heap.get_map_mut(r).remove(k.as_str()).unwrap_or(VmValue::Nil)),
            _ => Err(type_err("delete expects a String key")),
        },
        "merge" if args.len() == 1 => match &args[0] {
            VmValue::Map(other_r) => {
                let mut new_map = heap.get_map(r).clone();
                for (k, v) in heap.get_map(*other_r).iter() {
                    new_map.insert(k.clone(), v.clone());
                }
                Ok(VmValue::Map(heap.alloc(HeapObject::Map(new_map))))
            }
            _ => Err(type_err("merge expects a Map")),
        },
        "to_s" if args.is_empty() => Ok(VmValue::Str(format_value_with_heap(heap, recv))),
        _ => Err(type_err(&format!("Map has no method '{}'", name))),
    }
}

fn dispatch_range_method(
    heap: &mut GcHeap<HeapObject>,
    from: i64,
    to: i64,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    let type_err = |msg: &str| VmError::TypeError { message: msg.to_string(), line };
    match name {
        "size" if args.is_empty() => Ok(VmValue::Int((to - from).max(0))),
        "to_a" if args.is_empty() => {
            let v: Vec<VmValue> = (from..to).map(VmValue::Int).collect();
            Ok(VmValue::List(heap.alloc(HeapObject::List(v))))
        }
        "include?" if args.len() == 1 => match &args[0] {
            VmValue::Int(n) => Ok(VmValue::Bool(n >= &from && n < &to)),
            _ => Err(type_err("include? expects an Int")),
        },
        "first" if args.is_empty() => Ok(VmValue::Int(from)),
        "last" if args.is_empty() => Ok(VmValue::Int(to - 1)),
        "min" if args.is_empty() => Ok(VmValue::Int(from)),
        "max" if args.is_empty() => Ok(VmValue::Int(to - 1)),
        "to_s" if args.is_empty() => Ok(VmValue::Str(format!("{}", recv))),
        _ => Err(VmError::TypeError {
            message: format!("Range has no method '{}'", name),
            line,
        }),
    }
}
