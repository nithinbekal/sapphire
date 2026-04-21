# Adding a new stdlib class

Native-backed stdlib work: a Sapphire stub, a `native_*` module, `pub mod` in `lib.rs`, wiring in `vm.rs` (and `native_dispatch.rs` for value types). References: `Process` / `Env` (normal class), `Socket` (side-table + `&mut self` on `Vm`), `Set` (heap value type).

**Checklist:** stub → `dispatch_*` → `lib.rs` mod → `vm.rs` (`load_stdlib`, class dispatch, instance dispatch if needed) → tests → `./scripts/ci`.

---

## 1. Sapphire stub (`stdlib/src/foo.spr`)

Declares the class for the VM class table and global exposure. Method bodies live in Rust; the stub can be empty:

```sapphire
class Foo { }
```

If Rust builds instances, declare fields with `attr` so names match the keys you insert in Rust when constructing `VmValue::Instance` (see §4b):

```sapphire
class Foo {
  attr value
  attr code
}
```

Doc-comment style (see `math.spr`): one-line summary, blank `#`, indented examples, blank `#`, then `class` on the next line (no blank line before `class`):

```sapphire
# Foo provides access to the frobnication subsystem.
#
#   Foo.frob("thing")   # returns a Foo.Result
#
class Foo { }
```

### Nested classes

Use when a method returns a structured object. The VM registers the **simple** name (`"Result"` for `Foo.Result`); use that name in Rust when building instances.

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

---

## 2. Native dispatch (`src/native_foo.rs`)

For returns that are plain `VmValue` scalars only, mirror `dispatch_file_class_method`:

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

### List, Map, or Instance from class methods

`VmValue::List` / `Map` need heap allocation (`&mut self` on the VM). The standalone dispatcher cannot hold that, so return an intermediate enum and map it in `vm.rs`:

```rust
use std::collections::HashMap;
use crate::vm::{VmError, VmValue};

pub enum FooResult {
    Primitive(VmValue),
    List(Vec<VmValue>),
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

Add every new `native_*` file here once; later sections assume this.

---

## 4. Wire the VM (`src/vm.rs`)

### 4a. `load_stdlib`

```rust
("stdlib/foo.spr", include_str!("../stdlib/src/foo.spr")),
```

Place with similar utility classes (e.g. after `file.spr`, before `math.spr`). Any new `stdlib/*.spr` uses the same `include_str!` pattern.

### 4b. Class method branch

Add a peer of the `} else if name == "File" {` branch:

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
                .get("Result")
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

`alloc_list` / `alloc_map` / `alloc_fields` are existing `Vm` helpers. Nested class: `class_name` is the simple name (`"Result"`), not `"Foo.Result"`. Field `HashMap` keys match the stub `attr` names.

### 4c. Instance methods (optional)

If instances expose methods, add a block before `if dt_handled` in the native instance path (search for the `Instant` / datetime comment). Pattern: copy args from the stack, call `dispatch_foo_instance_method`, handle `VmError::Raised`, set a `*_handled` flag, join `foo_handled || dt_handled`. The dispatcher receives `GcRef` to fields — use `heap.get_fields(fields_ref)`.

---

## 5. Tests

**Sapphire** (`stdlib/tests/foo_test.spr`) when behaviour is easy to express in Sapphire:

```sapphire
class FooTest < Test {
  def test_frob_returns_result {
    r = Foo.frob("thing")
    assert_equal("ok", r.value)
    assert_equal(0, r.code)
  }
}
```

**Rust** (`tests/stdlib/foo.rs`) when wrapping I/O or OS resources; inject ports/paths with `format!` and `eval`. Register in `tests/stdlib.rs`:

```rust
#[path = "stdlib/foo.rs"]
mod foo;
```

---

## 6. Verify

```bash
./scripts/ci
```

All four checks (cargo test, clippy, sapphire test, examples) must pass.

---

## OS resource wrappers (sockets, processes, …)

When state cannot live as `VmValue` fields (`TcpStream`, `Child`, …), use a **side table**: `HashMap<i64, Resource>` + monotonic id on `Vm`; instances store the id (e.g. `fd`). See `Socket`.

Standalone `native_foo.rs` functions are insufficient when dispatch must read `self.classes` / `self.heap` and mutate allocation or the map — the borrow checker blocks that. Use private `impl Vm` methods (`dispatch_foo_class`, `dispatch_foo_instance`) and call them from the same class/instance sites as §4b / §4c.

**Borrowing:** clone field map first: `let fields = self.heap.get_fields(r).clone()` before `self.foos.get_mut(&fd)` so the heap borrow does not overlap the map borrow.

```rust
fn dispatch_foo_class(&mut self, method: &str, args: &[VmValue], line: u32)
    -> Result<VmValue, VmError>
{
    match method {
        "connect" => {
            let resource = native_foo::open(args, line)?;
            let id = self.next_foo_id;
            self.next_foo_id += 1;
            self.foos.insert(id, resource);
            let methods = self.classes.get("Foo")
                .map(|e| e.methods.clone())
                .ok_or_else(|| VmError::TypeError {
                    message: "Foo class not loaded".into(), line,
                })?;
            let mut fields = HashMap::new();
            fields.insert("fd".to_string(), VmValue::Int(id));
            let fields_ref = self.alloc_fields(fields);
            Ok(VmValue::Instance { class_name: "Foo".into(), fields: fields_ref, methods })
        }
        _ => Err(VmError::TypeError { message: format!("Foo has no class method '{}'", method), line }),
    }
}

fn dispatch_foo_instance(&mut self, fields_ref: GcRef, method: &str, args: &[VmValue], line: u32)
    -> Result<VmValue, VmError>
{
    let fields = self.heap.get_fields(fields_ref).clone();
    let fd = match fields.get("fd") {
        Some(VmValue::Int(n)) => *n,
        _ => return Err(VmError::TypeError { message: "invalid fd".into(), line }),
    };
    let resource = self.foos.get_mut(&fd)
        .ok_or_else(|| VmError::Raised(VmValue::Str(format!("fd {} is closed", fd))))?;
    match method {
        "close" => { self.foos.remove(&fd); Ok(VmValue::Nil) }
        _ => Err(VmError::TypeError { message: format!("Foo has no method '{}'", method), line }),
    }
}
```

---

## Heap-backed value types (`List` / `Map` / `Set` style)

For a first-class type backed by `GcRef` (not `Instance`), `Set` is the reference. Mirror how `List` / `Map` are threaded through the VM; the snippets below use `Set` as the running example.

### `HeapObject`, `VmValue`, tracing, formatting

```rust
// HeapObject — add next to List/Map
Set(Vec<VmValue>),

// HeapObject::trace
HeapObject::Set(v) => v.iter().for_each(|val| collect_refs(val, out)),

// collect_refs on VmValue
VmValue::List(r) | VmValue::Map(r) | VmValue::Set(r) => out.push(*r),

// VmValue
Set(GcRef),

// PartialEq
(VmValue::Set(a), VmValue::Set(b)) => a == b,

// fmt::Display
VmValue::Set(_) => write!(f, "<set>"),

// format_value_with_heap — follow List formatting; join elements with ", "
VmValue::Set(r) => {
    let parts: Vec<String> = heap
        .get_set(*r)
        .iter()
        .map(|el| format_value_with_heap(heap, el))
        .collect();
    format!("Set{{{}}}", parts.join(", "))
}
```

### `GcHeap` accessors and `alloc_set`

```rust
pub fn get_set(&self, r: GcRef) -> &Vec<VmValue> {
    match self.get(r) {
        HeapObject::Set(v) => v,
        _ => panic!("GcRef is not a Set"),
    }
}
pub fn get_set_mut(&mut self, r: GcRef) -> &mut Vec<VmValue> {
    match self.get_mut(r) {
        HeapObject::Set(v) => v,
        _ => panic!("GcRef is not a Set"),
    }
}

fn alloc_set(&mut self, v: Vec<VmValue>) -> VmValue {
    self.maybe_gc();
    VmValue::Set(self.heap.alloc(HeapObject::Set(v)))
}
```

### Native module (`src/native_set.rs`)

```rust
use crate::gc::{GcHeap, GcRef};
use crate::vm::{format_value_with_heap, HeapObject, VmError, VmValue};

pub fn dispatch_set_method(
    heap: &mut GcHeap<HeapObject>,
    r: GcRef,
    recv: &VmValue,
    name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<VmValue, VmError> {
    match name {
        "size" | "length" if args.is_empty() => Ok(VmValue::Int(heap.get_set(r).len() as i64)),
        _ => Err(VmError::TypeError {
            message: format!("Set has no method '{}'", name),
            line,
        }),
    }
}
```

Register with `pub mod native_set;` in `lib.rs` (§3).

### `native_dispatch.rs`

```rust
VmValue::Set(_) => Some("Set"),           // primitive_class_name
VmValue::Set(_) => "Set",                // value_type_name
VmValue::Set(r) => crate::native_set::dispatch_set_method(heap, *r, recv, name, args, line),
```

### Block methods

Add an arm in `dispatch_native_block_method` before the catch-all (copy the shape of `Set` / `each` in `vm.rs`).

### `OpCode::NewInstance`

`Foo.new(args)` hits `NewInstance`, not class-method dispatch. Guard at the **top** of that handler before normal construction. See `Foo.new` in `CLAUDE.md`.

```rust
if class_name == "Set" {
    let list_val = if n_pairs == 0 { None } else { Some(self.stack[base + 2].clone()) };
    let elements = match list_val {
        None => Vec::new(),
        Some(VmValue::List(lr)) => crate::native_set::dedup_list(self.heap.get_list(lr).clone()),
        _ => return Err(VmError::TypeError {
            message: "Set.new expects a List argument".to_string(),
            line,
        }),
    };
    self.stack.drain(base..);
    let result = self.alloc_set(elements);
    self.stack.push(result);
    continue;
}
```

`dedup_list` can live in `native_set.rs` (`VmValue: PartialEq` but not `Hash`).

### Stub and `load_stdlib`

Higher-order methods can live in Sapphire on top of a wired `each` block method:

```sapphire
class Set {
  def map() {
    result = []
    each { |x| result.append(yield(x)) }
    result
  }
}
```

Register the file in `load_stdlib` (§4a).

### Tests

`stdlib/tests/set_test.spr`: construction, membership, mutation, set algebra, `to_a` / `to_s`, `each`, and higher-order helpers from the stub.
