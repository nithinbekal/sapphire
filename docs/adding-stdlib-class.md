# Adding a new stdlib class

This guide walks through every step needed to add a new class to the Sapphire
standard library, backed by native Rust dispatch. The `Process` and `Env`
classes are the primary reference implementations.

---

## 1. Write the Sapphire stub (`stdlib/src/foo.spr`)

The file declares the class so it is registered in the VM class table and
exposed as a global. All method logic lives in Rust — the stub can be empty:

```sapphire
class Foo { }
```

Follow the doc-comment style used by `math.spr`: one-line description, a blank
`#` line, then indented usage examples, then a final blank `#` line, with the
`class` keyword immediately after (no blank line between):

```sapphire
# Foo provides access to the frobnication subsystem.
#
#   Foo.frob("thing")   # returns a Foo.Result
#
class Foo { }
```

### Nested classes

Sapphire supports nested class definitions natively. Use them when a method
returns a structured object rather than a primitive:

```sapphire
class Foo {
  # Holds the result of a Foo.frob call.
  #
  class Result {
    attr value: Str
    attr code: Int
  }
}
```

Access the nested class as `Foo.Result`. The simple name (`"Result"`) is what
the VM registers in `self.classes` and what you use in Rust when instantiating.

---

## 2. Write the native dispatch module (`src/native_foo.rs`)

Match the signature of `dispatch_file_class_method` when all return values are
primitives (`VmValue::Str`, `VmValue::Int`, `VmValue::Bool`, `VmValue::Nil`):

```rust
use crate::vm::{VmError, VmValue};

pub fn dispatch_foo_class_method(
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match name {
        "frob" => { /* … */ }
        _ => Err(VmError::TypeError {
            message: format!("Foo has no class method '{}'", name),
            line,
        }),
    }
}
```

### Returning a List, Map, or Instance

`VmValue::List` and `VmValue::Map` require GC heap allocation, which needs
`&mut self` on the VM. The dispatch function cannot hold that reference, so
return an intermediate enum and let `vm.rs` do the allocation:

```rust
use std::collections::HashMap;
use crate::vm::{VmError, VmValue};

pub enum FooResult {
    Primitive(VmValue),
    List(Vec<VmValue>),
    /// Fields for a Foo.Result instance.
    Output { value: String, code: i64 },
}

pub fn dispatch_foo_class_method(
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<FooResult, VmError> {
    match name {
        "items" => Ok(FooResult::List(vec![/* … */])),
        "frob"  => Ok(FooResult::Output { value: "ok".into(), code: 0 }),
        _ => Err(VmError::TypeError {
            message: format!("Foo has no class method '{}'", name),
            line,
        }),
    }
}
```

---

## 3. Register the module (`src/lib.rs`)

```rust
pub mod native_foo;
```

---

## 4. Wire into the VM (`src/vm.rs`)

Two edits are required.

### 4a. Add the source file to `load_stdlib`

```rust
("stdlib/foo.spr", include_str!("../stdlib/src/foo.spr")),
```

Place it near the other utility classes (after `file.spr`, before `math.spr`).

### 4b. Add a dispatch branch in the class method chain

Find the `} else if name == "File" {` branch and add a peer branch after it:

```rust
} else if name == "Foo" {
    let foo_args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
    let foo_result = match crate::native_foo::dispatch_foo_class_method(
        &method_name, &foo_args, line,
    ) {
        Ok(r) => r,
        Err(VmError::Raised(val)) => {
            self.stack.truncate(recv_slot);
            self.raise_value(val)?;
            continue;
        }
        Err(e) => return Err(e),
    };
    let result = match foo_result {
        crate::native_foo::FooResult::Primitive(v) => v,
        crate::native_foo::FooResult::List(items) => self.alloc_list(items),
        crate::native_foo::FooResult::Output { value, code } => {
            let methods = self
                .classes
                .get("Result")          // simple name of the nested class
                .map(|e| e.methods.clone())
                .ok_or_else(|| VmError::TypeError {
                    message: "Foo.Result class not loaded".to_string(),
                    line,
                })?;
            let mut fields = HashMap::new();
            fields.insert("value".to_string(), VmValue::Str(value));
            fields.insert("code".to_string(),  VmValue::Int(code));
            let gc_fields = self.alloc_fields(fields);
            VmValue::Instance { class_name: "Result".to_string(), fields: gc_fields, methods }
        }
    };
    self.stack.truncate(recv_slot);
    self.stack.push(result);
```

**Key points:**
- `self.alloc_list(v)` and `self.alloc_map(m)` are helpers already on `Vm`.
- `self.alloc_fields(m)` allocates a `HeapObject::Fields` for an instance.
- The `class_name` in `VmValue::Instance` must match the name the class was
  compiled under — for a nested class `Foo { class Result { } }` that is
  `"Result"`, not `"Foo.Result"`.
- Field names in the `HashMap` must exactly match the `attr` names declared in
  the `.spr` class body.

---

## 5. Write tests (`stdlib/tests/foo_test.spr`)

```sapphire
class FooTest < Test {
  def test_frob_returns_result {
    r = Foo.frob("thing")
    assert_equal("ok", r.value)
    assert_equal(0, r.code)
  }
}
```

---

## 6. Verify

```bash
./scripts/ci
```

All four checks (cargo test, clippy, sapphire test, examples) must pass.
