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

If Rust will create instances of this class (i.e. the class has instance
methods, not just class methods), declare the fields with `attr` so the
field names are visible to the VM and match what Rust inserts:

```sapphire
class Foo {
  attr value
  attr code
}
```

The `attr` names must exactly match the `HashMap` keys your Rust code
inserts when building the instance. See step 4b.

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

### 4c. Instance method dispatch (if the class returns instances)

If your class creates `VmValue::Instance` values and those instances have
methods, wire a second dispatch branch into the instance method section of
`vm.rs`. Find the block that begins:

```rust
// For datetime types, try native dispatch first …
let mut dt_handled = false;
if matches!(dt_class.as_str(), "Instant" | …) {
```

Add a peer block immediately before the `if dt_handled {` line:

```rust
let mut foo_handled = false;
if dt_class == "Foo" {
    let args: Vec<VmValue> = self.stack[recv_slot + 1..].to_vec();
    match crate::native_foo::dispatch_foo_instance_method(dt_fields, &method_name, &args, line) {
        Ok(result) => {
            self.stack.truncate(recv_slot);
            self.stack.push(result);
            foo_handled = true;
        }
        Err(VmError::Raised(val)) => {
            self.stack.truncate(recv_slot);
            self.raise_value(val)?;
            continue;
        }
        Err(e) => return Err(e),
    }
}

if foo_handled || dt_handled {
    // already handled
} else if let Some(method) = method_opt {
```

The dispatch function receives `dt_fields: GcRef` — read the instance's
field map with `heap.get_fields(fields_ref)` (requires `&GcHeap` access).

---

## 5. Write tests

**Sapphire tests** (`stdlib/tests/foo_test.spr`) are the default for classes
whose behaviour can be exercised from Sapphire:

```sapphire
class FooTest < Test {
  def test_frob_returns_result {
    r = Foo.frob("thing")
    assert_equal("ok", r.value)
    assert_equal(0, r.code)
  }
}
```

**Rust integration tests** (`tests/stdlib/foo.rs`) are preferable when the
class wraps I/O or OS resources (sockets, file descriptors, subprocesses)
where a Sapphire-level test would need live external access. Spin up a
controlled environment (loopback server, temp file, etc.) in a thread, then
call `eval` with the port/path injected via `format!`:

```rust
#[test]
fn foo_does_thing() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || { /* serve one request */ });
    let src = format!(r#"Foo.connect("127.0.0.1", {port}).read_all()"#);
    assert_eq!(eval(&src), VmValue::Str("response".into()));
}
```

Wire the file into `tests/stdlib.rs`:

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

## Classes that wrap OS resources (sockets, file handles, processes)

Some classes can't store their state as `VmValue` fields because the
underlying Rust type (e.g. `TcpStream`, `Child`) isn't serialisable as a
`VmValue`. The `Socket` class is the reference implementation.

### The side-table pattern

Keep a `HashMap<i64, Resource>` on the `Vm` struct alongside a monotonic
integer counter. Each instance stores only the integer key in an `fd` field;
native dispatch looks up the real resource by that key.

**`src/vm.rs` — add to `Vm` struct:**

```rust
sockets: HashMap<i64, std::io::BufReader<std::net::TcpStream>>,
next_socket_id: i64,
```

Initialise both in `Vm::new` and `Vm::new_repl`.

### Why dispatch must be `&mut self` methods on `Vm`

A standalone function in `native_foo.rs` can't be used when the dispatch
needs to both read VM state (`self.classes`, `self.heap`) and mutate it
(`self.alloc_fields`, `self.sockets`). The Rust borrow checker disallows
holding a shared reference to one field while calling a `&mut self` method.

The solution is to make the dispatch logic a private `impl Vm` method:

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
    // Clone fields first to drop the immutable borrow before touching self.foos.
    let fields = self.heap.get_fields(fields_ref).clone();
    let fd = match fields.get("fd") {
        Some(VmValue::Int(n)) => *n,
        _ => return Err(VmError::TypeError { message: "invalid fd".into(), line }),
    };
    let resource = self.foos.get_mut(&fd)
        .ok_or_else(|| VmError::Raised(VmValue::Str(format!("fd {} is closed", fd))))?;
    match method {
        "close" => { self.foos.remove(&fd); Ok(VmValue::Nil) }
        // … other arms call free functions in native_foo.rs …
        _ => Err(VmError::TypeError { message: format!("Foo has no method '{}'", method), line }),
    }
}
```

**Key constraint:** call `self.heap.get_fields(r).clone()` to extract the fd
*before* calling `self.foos.get_mut(…)`. The clone ends the immutable borrow
on `self.heap`, allowing the subsequent mutable borrow of `self.foos`.

Wire the `&mut self` class method into the class dispatch chain (step 4b) and
the instance method into the instance dispatch section (step 4c), passing
`self.dispatch_foo_class(…)` and `self.dispatch_foo_instance(…)` respectively.

---

## Adding a value type

Use this section when you want to add a first-class collection or value type —
one backed by `GcRef` rather than an `Instance` — like `List`, `Map`, or `Set`.
The `Set` type is the primary reference implementation.

### Step 1 — `HeapObject` variant (`src/vm.rs`)

Add the variant after the existing collection variants:

```rust
pub enum HeapObject {
    List(Vec<VmValue>),
    Map(HashMap<String, VmValue>),
    Set(Vec<VmValue>),    // ← example
    Fields(HashMap<String, VmValue>),
}
```

Update the `Trace` impl inside `HeapObject::trace`:

```rust
HeapObject::Set(v) => v.iter().for_each(|val| collect_refs(val, out)),
```

Update `collect_refs()` — add the new variant alongside the other `GcRef` carriers:

```rust
VmValue::List(r) | VmValue::Map(r) | VmValue::Set(r) => out.push(*r),
```

### Step 2 — `VmValue` variant (`src/vm.rs`)

Add the variant after the existing collection variants:

```rust
pub enum VmValue {
    // … existing variants …
    Set(GcRef),
}
```

Add an arm to `PartialEq`:

```rust
(VmValue::Set(a), VmValue::Set(b)) => a == b,
```

Add an arm to `fmt::Display` (short fallback used in error messages):

```rust
VmValue::Set(_) => write!(f, "<set>"),
```

Add an arm to `format_value_with_heap()` (used by `to_s` and the REPL):

```rust
VmValue::Set(r) => {
    let parts: Vec<String> = heap
        .get_set(*r)
        .iter()
        .map(|el| format_value_with_heap(heap, el))
        .collect();
    format!("Set{{{}}}", parts.join(", "))
}
```

### Step 3 — `GcHeap` accessors and `alloc_*` helper (`src/vm.rs`)

Add immutable and mutable accessors on `GcHeap<HeapObject>`:

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
```

Add a private `alloc_set` helper on `Vm` (alongside `alloc_list`, `alloc_map`):

```rust
fn alloc_set(&mut self, v: Vec<VmValue>) -> VmValue {
    self.maybe_gc();
    VmValue::Set(self.heap.alloc(HeapObject::Set(v)))
}
```

### Step 4 — Native dispatch module (`src/native_set.rs`)

Create `src/native_set.rs`. It receives a shared `&mut GcHeap<HeapObject>` so it
can allocate new values while reading existing ones:

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
        // … more arms …
        _ => Err(VmError::TypeError {
            message: format!("Set has no method '{}'", name),
            line,
        }),
    }
}
```

Register the module in `src/lib.rs`:

```rust
pub mod native_set;
```

### Step 5 — Wire into `native_dispatch.rs` (`src/native_dispatch.rs`)

Three additions:

```rust
// primitive_class_name()
VmValue::Set(_) => Some("Set"),

// value_type_name()
VmValue::Set(_) => "Set",

// dispatch_native_method() match arm
VmValue::Set(r) => crate::native_set::dispatch_set_method(heap, *r, recv, name, args, line),
```

### Step 6 — Block method dispatch (`src/vm.rs`)

Add an arm inside `dispatch_native_block_method` before the `other =>` catch-all:

```rust
VmValue::Set(r) => {
    let r = *r;
    match name {
        "each" => {
            let items: Vec<VmValue> = self.heap.get_set(r).clone();
            for item in items {
                match self.call_block(&blk, vec![item]) {
                    Err(VmError::Next(_)) => continue,
                    Err(VmError::Break(v)) => return Ok(v),
                    Err(e) => return Err(e),
                    Ok(_) => {}
                }
            }
            Ok(recv.clone())
        }
        _ => Err(VmError::TypeError {
            message: format!("Set has no block method '{}'", name),
            line,
        }),
    }
}
```

### Step 7 — Intercept `OpCode::NewInstance` (`src/vm.rs`)

> **Important:** `Foo.new(args)` compiles to `OpCode::NewInstance`, not
> `Invoke("new", …)`. The class-method dispatch chain is never reached for
> `new`. See the `Foo.new(args)` bullet in CLAUDE.md for details.

Add a guard at the **top** of the `OpCode::NewInstance` handler, before the
normal instance-creation path:

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

The `dedup_list` helper (a free function in `native_set.rs`) deduplicates items
using linear `contains()` — `VmValue` implements `PartialEq` but not `Hash`.

### Step 8 — Sapphire stub and stdlib registration

For higher-order methods (those that take blocks), write them in Sapphire and
rely on the `each` block method you wired in Step 6:

```sapphire
class Set {
  def map() {
    result = []
    each { |x| result.append(yield(x)) }
    result
  }
  # select, reject, any?, all?, none?, each_with_index follow the same pattern
}
```

Register the file in `vm.load_stdlib()`:

```rust
("stdlib/set.spr", include_str!("../stdlib/src/set.spr")),
```

### Step 9 — Tests

Create `stdlib/tests/set_test.spr` extending `Test`. Cover: construction
(empty, from list, deduplication), membership (`include?`), mutation (`add`,
`delete`), set algebra (`union`, `intersection`, `difference`, `subset?`,
`superset?`, `disjoint?`), conversion (`to_a`, `to_s`), `each`, and all
higher-order methods from the Sapphire stub.
