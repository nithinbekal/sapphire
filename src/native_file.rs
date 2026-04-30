use crate::gc::{GcHeap, GcRef};
use crate::vm::{define_native_class_method, HeapObject, VmError, VmValue};
use VmValue::Str;

pub fn register_class_methods(heap: &mut GcHeap<HeapObject>, class_ref: GcRef) {
    define_native_class_method(heap, class_ref, "exist?", 1, file_exist_q);
    define_native_class_method(heap, class_ref, "read", 1, file_read);
    define_native_class_method(heap, class_ref, "write", 2, file_write);
}

fn file_exist_q(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [Str(path)] => Ok(VmValue::Bool(std::path::Path::new(path.as_str()).exists())),
        [_] => Err(VmError::TypeError {
            message: "File.exist?: path must be a string".to_string(),
            line,
        }),
        _ => Err(VmError::TypeError {
            message: format!("File.exist? expects 1 argument, got {}", args.len()),
            line,
        }),
    }
}

fn file_read(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [Str(path)] => std::fs::read_to_string(path.as_str())
            .map(Str)
            .map_err(|e| VmError::Raised(Str(format!("{path}: {e}")))),
        [_] => Err(VmError::TypeError {
            message: "File.read: path must be a string".to_string(),
            line,
        }),
        _ => Err(VmError::TypeError {
            message: format!("File.read expects 1 argument, got {}", args.len()),
            line,
        }),
    }
}

fn file_write(
    _heap: &mut GcHeap<HeapObject>,
    _recv: &VmValue,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match args {
        [Str(path), Str(content)] => std::fs::write(path.as_str(), content.as_str())
            .map(|_| VmValue::Nil)
            .map_err(|e| VmError::Raised(Str(format!("{path}: {e}")))),
        [_, _] => Err(VmError::TypeError {
            message: "File.write: path and content must be strings".to_string(),
            line,
        }),
        _ => Err(VmError::TypeError {
            message: format!("File.write expects 2 arguments, got {}", args.len()),
            line,
        }),
    }
}
