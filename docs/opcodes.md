# Bytecode opcodes

Reference for the Sapphire VM’s **`OpCode`** values and their **stack effects**. It assumes you are already comfortable with **stack machines** and bytecode in general; for how source becomes a `Chunk`, see **`docs/architecture.md`**. Authoritative definitions: **`src/chunk.rs`** (`OpCode`, `Constant`, `Chunk`); execution: **`src/vm.rs`**.

Each compiled function owns a **`Chunk`**:

- **`code`** — `Vec<OpCode>`
- **`constants`** — literals and embedded `Function` / `ClassDesc` / …
- **`lines`** — source line per instruction (errors)

**`Chunk::disassemble`** prints a readable listing.

## Operand stack

Most opcodes **consume** operands from the **top** of the stack (last pushed, first popped) and **push** results. Calls, methods, and class-related opcodes use **fixed layouts** called out below; when in doubt, match on `OpCode` in `vm.rs`.

```mermaid
flowchart LR
  S1["… 1 2 ← top"] --> ADD[Add]
  ADD --> S2["… 3 ← top"]
```

**Jump offsets** (`chunk.rs`): forward jumps skip **`offset`** instructions **starting after** the jump opcode. **`Loop(offset)`** moves **`ip`** backward by **`offset`**.

## Literals and constants

| Opcode | Effect |
|--------|--------|
| `Constant(idx)` | Push `constants[idx]`. |
| `Closure(idx)` | Build closure from `Function` at `idx`, wiring upvalues per `upvalue_defs`. |
| `True` / `False` / `Nil` | Push boolean or nil. |

## Arithmetic and bitwise

`Add`, `Sub`, `Mul`, `Div`, `Mod` — binary numeric ops.  
`BitAnd`, `BitOr`, `BitXor`, `BitNot`, `Shl`, `Shr` — integer bitwise / shifts.  
`Negate`, `Not` — unary.

## Comparison

`Equal`, `NotEqual`, `Less`, `LessEqual`, `Greater`, `GreaterEqual`.

## Locals and upvalues

| Opcode | Effect |
|--------|--------|
| `GetLocal(slot)` / `SetLocal(slot)` | Frame-local stack slot. |
| `GetUpvalue(i)` / `SetUpvalue(i)` | Enclosed variable cell. |
| `CloseUpvalue` | Close open upvalue for TOS slot, then pop. |

## Control flow

| Opcode | Effect |
|--------|--------|
| `Jump(offset)` | Unconditional forward jump. |
| `JumpIfFalse(offset)` | Pop; if falsy, jump forward. |
| `Loop(offset)` | Backward jump for loops. |
| `JumpIfFalseKeep(offset)` | Short-circuit **`and`**: if TOS falsy, jump keeping TOS; else pop, continue. |
| `JumpIfTrueKeep(offset)` | Short-circuit **`or`**: if TOS truthy, jump keeping TOS; else pop, continue. |

## Calls

| Opcode | Effect |
|--------|--------|
| `Call(argc)` | Callable is `argc` slots below TOS; args on top. |
| `CallWithBlock(argc)` | Like `Call` but stack is `[..., fn, block, args...]`. |
| `Invoke(name_idx, argc)` | Method dispatch: receiver below args; `name_idx` is `Str` constant. |
| `InvokeWithBlock(name_idx, argc)` | `Invoke` plus block argument. |
| `SuperInvoke(name_idx, argc)` | Superclass method dispatch. |
| `Yield(argc)` | Invoke the block passed into the current function (dedicated upvalue). |
| `Return` | Return from current function. |
| `NonLocalReturn` | `return` from inside a block invoked by native code; surfaces as `VmError::Return` in that frame. |

## Strings and collections

| Opcode | Effect |
|--------|--------|
| `BuildString(n)` | Pop `n` values, stringify and concatenate, push `Str`. |
| `BuildList(n)` | Pop `n` values, push list (order preserved). |
| `BuildMap(n)` | Pop `n` key/value pairs (keys are `Str`), push map. |
| `BuildRange` | Pop `to`, then `from` (ints), push `Range`. |

## Indexing

| Opcode | Effect |
|--------|--------|
| `Index` | Pop index, then container; push element. |
| `IndexSet` | Pop value, index, container; assign; push value. |

## Classes and instances

| Opcode | Effect |
|--------|--------|
| `DefClass(const_idx)` | Pop method closures per `ClassDesc.method_names`, optional nested classes, optional dynamic superclass; push `Class`. |
| `NewInstance(n_pairs)` | Pop `class`, then alternating field names and values; allocate instance. **`Foo.new` is compiled to this**, not `Invoke("new", …)`. |
| `GetField(idx)` / `SetField(idx)` | Instance field by `Str` constant. |
| `GetFieldSafe(idx)` | Like `GetField` but safe on nil receiver (nil). |
| `GetSelf` | Push `self` (slot 0) for `SelfExpr`. |

## Exceptions and non-local exit

| Opcode | Effect |
|--------|--------|
| `Raise` | Pop raised value; unwind to `BeginRescue` handler. |
| `Break` | Pop value; unwind block caller. |
| `Next` | Pop value; non-local return from block frame. |
| `BeginRescue { handler_offset, rescue_var_slot }` | Register handler; offset patched when body length known (`Chunk::patch_rescue`). |
| `PopRescue` | Handler scope ends normally. |

## Built-in helpers

Used by stdlib / fast paths: `Len`, `MapKeys`, `RangeFrom`, `RangeTo`.

## I/O and modules

| Opcode | Effect |
|--------|--------|
| `Print` | Pop value, print with newline, push `Nil`. |
| `Import(path_idx)` | Load and run file (relative path constant). |

## REPL globals

| Opcode | Effect |
|--------|--------|
| `GetGlobal(idx)` | Push global named by `Str` constant at `idx`. |
| `SetGlobal(idx)` | Store TOS into global (peek, no pop). |

## Stack

`Pop` — discard TOS.

## Related code

- `src/chunk.rs` — `OpCode`, `Constant`, `Chunk`, patching jumps/rescue
- `src/compiler.rs` — opcode selection and special cases
- `src/vm.rs` — interpreter loop (`OpCode` match)
