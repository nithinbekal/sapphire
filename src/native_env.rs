use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_class_method, HeapObject, VmError, VmValue};
use VmValue::Str;

pub fn register_class_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_class_method(heap, class_ref, "all", 0, env_all);
    define_native_class_method(heap, class_ref, "delete", 1, env_delete);
    define_native_class_method(heap, class_ref, "fetch", 1, env_fetch);
    define_native_class_method(heap, class_ref, "get", 1, env_get);
    define_native_class_method(heap, class_ref, "set", 2, env_set);
}

fn env_all(
    heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    _args: &[VmValue],
    _line: u32,
) -> Result<VmValue, VmError> {
    let vars: std::collections::HashMap<String, VmValue> =
        std::env::vars().map(|(k, v)| (k, VmValue::Str(v))).collect();
    Ok(VmValue::Map(heap.alloc(HeapObject::Map(vars))))
}

fn env_delete(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [Str(var_name)] => {
            // SAFETY: only called from single-threaded Sapphire VM
            unsafe { std::env::remove_var(var_name.as_str()) };
            Ok(VmValue::Nil)
        }
        [_] => Err(VmError::TypeError {
            message: "Env.delete: name must be a string".to_string(),
            line,
        }),
        _ => Err(VmError::TypeError {
            message: format!("Env.delete expects 1 argument, got {}", args.len()),
            line,
        }),
    }
}

fn env_fetch(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [Str(var_name)] => std::env::var(var_name.as_str())
            .map(Str)
            .map_err(|_| VmError::Raised(Str(format!("environment variable not found: {var_name}")))),
        [_] => Err(VmError::TypeError {
            message: "Env.fetch: name must be a string".to_string(),
            line,
        }),
        _ => Err(VmError::TypeError {
            message: format!("Env.fetch expects 1 argument, got {}", args.len()),
            line,
        }),
    }
}

fn env_get(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [Str(var_name)] => {
            Ok(std::env::var(var_name.as_str()).map(Str).unwrap_or(VmValue::Nil))
        }
        [_] => Err(VmError::TypeError {
            message: "Env.get: name must be a string".to_string(),
            line,
        }),
        _ => Err(VmError::TypeError {
            message: format!("Env.get expects 1 argument, got {}", args.len()),
            line,
        }),
    }
}

fn env_set(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [Str(var_name), Str(value)] => {
            // SAFETY: only called from single-threaded Sapphire VM
            unsafe { std::env::set_var(var_name.as_str(), value.as_str()) };
            Ok(VmValue::Nil)
        }
        [_, _] => Err(VmError::TypeError {
            message: "Env.set: name and value must be strings".to_string(),
            line,
        }),
        _ => Err(VmError::TypeError {
            message: format!("Env.set expects 2 arguments, got {}", args.len()),
            line,
        }),
    }
}
