# AGENTS.md

Guidance for AI coding agents in this repository. For Claude Code, see `CLAUDE.md`.

## When finishing work

Run **`./scripts/ci`** before you consider a task done. It runs `cargo test`, clippy, `sapphire test` on stdlib tests, and the examples smoke script.

For quick iteration: `cargo test`, `cargo test <name> -- --nocapture`, `sapphire test [path]` (recursive `*_test.spr`).

```bash
cargo build
cargo test
cargo test <test_name> -- --nocapture
sapphire test [path]
./scripts/ci
```

## What Sapphire is

Ruby-inspired, gradually typed, object-oriented scripting in Rust. User code compiles to bytecode and runs on a stack VM (no AST interpreter on the hot path). CLI: `run`, `typecheck`, `test`, `console`, `version`.

**Pipeline:** Lexer → Parser → Compiler → VM. Same path for `run`, tests, and REPL.

**Where things live:** `src/main.rs` (CLI), `lexer` / `parser` / `ast` / `compiler` / `chunk` / `vm` / `typechecker` / `native` (+ `native_*`), `stdlib/` embedded and loaded via `vm.load_stdlib()`. Module-by-module map: [`docs/architecture.md`](docs/architecture.md). Opcodes: [`docs/opcodes.md`](docs/opcodes.md). GC: [`docs/gc.md`](docs/gc.md).

**Values:** Runtime values are `VmValue` in `src/vm.rs`. Chunk constant pool uses the smaller `Value` in `src/value.rs`. Heap layout details: `docs/gc.md` and `vm.rs`.

## Testing

- **Rust:** Integration tests under `tests/`; helpers like `eval`, `eval_with_stdlib`, `eval_err` are common.
- **Sapphire:** Files ending in `_test.spr`, classes subclass `Test`; assertions live in `stdlib/src/test.spr`. Runner: `sapphire test [path]` (`.` pass, `F` fail, summary at end).

## Compiler / VM gotchas

- `Call { callee: Get { object, name }, args }` → `OpCode::Invoke(name, arg_count)`; `Call` on a variable → `OpCode::Call` after pushing callee.
- Zero-arg calls: `obj.foo` and `obj.foo()` parse the same; `def foo { }` and `def foo() { }` the same for zero-arg defs.
- `Expr::Get` without call appears mainly as lvalue / non-call contexts.
- **`Foo.new(args)` → `OpCode::NewInstance`**, not `Invoke("new", …)` (`compiler.rs`). Class-method dispatch in `vm.rs` is not used for `new`; hook construction in the `NewInstance` handler.

## Language constraints

- No globals, class vars, or metaprogramming.
- Top-level `def` becomes `Object` methods.
- Primitives get methods via stdlib classes.
- Single inheritance; `defp` private; `self { }` for class methods.
- Gradual types: optional annotations, runtime-checked when present.
- Imports: relative (`./`, `../`), `.spr` implied, each file runs once.

## Standard library

Sources under `stdlib/` (mostly `stdlib/src/`), embedded and loaded in `load_stdlib()`. New primitive-backed classes: [`docs/adding-stdlib-class.md`](docs/adding-stdlib-class.md).
