# Adding a new stdlib class

There are two shapes of native-backed stdlib work:

1. **Normal class** — Instances are `VmValue::Instance` (or class methods only). Pattern: Sapphire stub, `native_*` class (and optional instance) dispatch, `pub mod` in `lib.rs`, `vm.rs` branches for class/instance. References: `Process` / `Env`, `Socket` (side-table + `&mut self` on `Vm`).

2. **Heap primitive + `ClassObject`** — Values are `VmValue::Foo(GcRef)` on the GC heap, but the **global** for `Foo` is `VmValue::ClassObj` so `receiver.class`, `.name`, and a **single method table** (native + Sapphire bytecode) work like Ruby. Reference: **`Set`**. Native instance methods use `define_native_method` / `register_methods`; they are **not** wired through `dispatch_native_method`.

**Checklist (normal):** stub → `dispatch_*` → `lib.rs` mod → `vm.rs` (`load_stdlib`, class dispatch, instance dispatch if needed) → tests → `./scripts/ci`.

**Checklist (heap + ClassObject):** stub → heap `VmValue` / `HeapObject` → `CoreClasses` + `bootstrap_core_classes` + `find_core_class_obj` + `gc_roots` → `native_*::register_methods` (`define_native_method`) → `OpCode::Invoke` / `InvokeWithBlock` / `.class` (copy `Set`) → `load_stdlib` global overwrite → optional `NewInstance` / class-method branch → `primitive_class_name` only (no `dispatch_native_method` arm) → tests → `./scripts/ci`.

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

For **class** methods (and normal classes’ instance dispatchers), use free functions or small enums. For **heap primitives on a `ClassObject`**, put **instance** natives in the same module as **`register_methods`** (see [Heap-backed primitives](#heap-backed-primitives-classobject-model)); skip this section’s `dispatch_foo_class_method` unless `Foo` also has class methods handled in `vm.rs`.

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

**`VmValue::Instance`:** add a block before `if dt_handled` in the native instance path (search for the `Instant` / datetime comment). Pattern: copy args from the stack, call `dispatch_foo_instance_method`, handle `VmError::Raised`, set a `*_handled` flag, join `foo_handled || dt_handled`. The dispatcher receives `GcRef` to fields — use `heap.get_fields(fields_ref)`.

**Heap primitive on a `ClassObject`:** instance methods are resolved in `OpCode::Invoke` via `lookup_class_object_method` (see [Heap-backed primitives](#heap-backed-primitives-classobject-model)); do not add a datetime-style `Vm` branch unless you still use the legacy path.

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

## Heap-backed primitives (ClassObject model)

Use this when the value is a **`GcRef` into `HeapObject`** (like `List` / `Map` / `Set`), and you want **`value.class` → `VmValue::ClassObj`**, **`Foo` global → `ClassObj`**, and **native + `.spr` bytecode in one method table** on `HeapObject::ClassObject`.

`List` and `Map` today still use the older path (`dispatch_native_method` + `ClassEntry` only). **`Set`** is the reference for the ClassObject model.

### 1. `HeapObject`, `VmValue`, trace roots, display

Add the payload variant, trace it in `Trace for HeapObject`, push the ref from `collect_refs` / `PartialEq` / `Display`, and extend `format_value_with_heap` (copy `Set`).

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

// format_value_with_heap
VmValue::Set(r) => {
    let parts: Vec<String> = heap
        .get_set(*r)
        .iter()
        .map(|el| format_value_with_heap(heap, el))
        .collect();
    format!("Set{{{}}}", parts.join(", "))
}
```

### 2. `GcHeap` accessors and `alloc_*`

```rust
pub fn get_set(&self, r: GcRef) -> &Vec<VmValue> { /* match HeapObject::Set */ }
pub fn get_set_mut(&mut self, r: GcRef) -> &mut Vec<VmValue> { /* … */ }

fn alloc_set(&mut self, v: Vec<VmValue>) -> VmValue {
    self.maybe_gc();
    VmValue::Set(self.heap.alloc(HeapObject::Set(v)))
}
```

### 3. `CoreClasses`, `gc_roots`, `bootstrap_core_classes`

- Add **`set_cls: Option<GcRef>`** (or `foo_cls`) on **`CoreClasses`** in `vm.rs`.
- **`gc_roots`:** include the new `Option` in the root list so the `ClassObject` is never collected.
- **`bootstrap_core_classes`** (runs at the start of `load_stdlib`):
  1. Allocate **`HeapObject::ClassObject`** for your type: `name: "Set".into()`, **`superclass: Some(object)`** (the bootstrapped Object ref), **`class_ref: None`**, empty **`methods`**.
  2. Extend the **`class_ref` fixup** loop so every bootstrapped `ClassObject` (including yours) gets **`class_ref = Some(class_cls)`**.
  3. Store **`Some(set_cls)`** on **`self.core_classes`**.
  4. Call **`crate::native_set::register_methods(&mut self.heap, set_cls)`** so natives land in **`methods`** before `.spr` runs.

### 4. `find_core_class_obj` and `load_stdlib` globals

- **`find_core_class_obj`:** return **`Some(set_cls)`** for **`"Set"`** so `OpCode::DefClass` can mirror bytecode into the same `ClassObject` after each stdlib file runs.
- **`load_stdlib`:** after the loop that fills **`globals`** with **`VmValue::Class`**, **overwrite** **`"Set"`** with **`VmValue::ClassObj(set_cls)`** so user code and **`Set.new`** see the heap class object.

**DefClass mirroring:** merged instance methods from `.spr` are inserted into the `ClassObject`. If a name is already **`SapphireMethod::Native`** and the merged method is **only inherited** (`vm_method.defined_in != class_name`), the mirror **skips** the insert so natives like **`Set#to_s`** are not replaced by **`Object#to_s`**.

### 5. Native module — `define_native_method` + `register_methods`

In **`src/native_set.rs`** (or `native_foo.rs`):

- Each method is a **`pub fn`** with type **`NativeFn`**:

  `fn(&mut GcHeap<HeapObject>, &VmValue, &[VmValue], u32) -> Result<VmValue, VmError>`

  Unpack **`VmValue::Set(r)`** (or your variant) from **`recv`**, then implement the body.

- **`register_methods(heap, class_ref)`** calls **`crate::vm::define_native_method(heap, class_ref, "add", 1, set_add)`** for each name/arity/function.

- **`pub mod native_set;`** in **`lib.rs`**.

- **`primitive_class_name`** / **`value_type_name`** in **`native.rs`**: add **`Set`** (or **`Foo`**) so **`ClassEntry`** fallback and errors still know the type. **Do not** add a **`VmValue::Set`** arm to **`dispatch_native_method`** for types fully on the ClassObject path.

### 6. `OpCode::Invoke` and `InvokeWithBlock`

Copy the **`Set`** blocks in **`vm.rs`**:

- **Non-block:** `matches!(recv, VmValue::Set(_)) && let Some(set_cls) = self.core_classes.set_cls && let Some(m) = self.lookup_class_object_method(set_cls, &method_name)` — dispatch **`SapphireMethod::Native`** (arity check + call **`func`**) or **`Bytecode`** (private + arity + **`CallFrame`**), same as **`ClassEntry`** path.
- **With block:** same **`&& let` chain** but match **`SapphireMethod::Bytecode(m)`** only; native **block** methods (e.g. **`each`**) stay in **`dispatch_native_block_method`** on **`Vm`**.

Then **`try_native_method`** / **`ClassEntry`** remain fallbacks for anything the chain does not define.

### 7. `.class` and class methods

- **`Invoke`**, **`method_name == "class"`**: for **`VmValue::Set`**, push **`VmValue::ClassObj(self.core_classes.set_cls)`** (same idea as current **`Set`** handling).
- **`Set.new`:** either a dedicated **`else if name == "Set"`** branch on **`VmValue::Class`** (legacy global during load) **or** rely on **`NewInstance`** once **`Set`** is **`ClassObj`** — **`NewInstance`** already resolves **`VmValue::ClassObj`** via **`ClassEntry`** and can special-case **`class_name == "Set"`** at the top (see existing code).

### 8. Block-only natives

Add an arm in **`dispatch_native_block_method`** on **`Vm`** (copy **`Set` / `each`**).

### 9. Stub and `load_stdlib`

Higher-order helpers in Sapphire call primitives you provide from Rust or from **`each`**:

```sapphire
class Set {
  def map {
    result = []
    each { |x| result.append(yield(x)) }
    result
  }
}
```

Register **`stdlib/src/set.spr`** in **`load_stdlib`** (§4a).

### 10. Tests

`stdlib/tests/set_test.spr` (or your type): construction, core natives, `to_s` / `to_a`, **`each`**, helpers from the stub, and **`value.class.name`** if you expose **`ClassObj`**.
