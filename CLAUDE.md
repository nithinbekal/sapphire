# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## After completing a task

Always run `./scripts/ci` when done with a task. It runs all CI checks (cargo test, clippy, sapphire tests, examples) and reports pass/fail for each.

## What is Sapphire?

Sapphire is an object-oriented scripting language implemented in Rust. It's Ruby-inspired with gradual typing. The CLI supports five main subcommands: `run`, `typecheck`, `test`, `console` (REPL), and `version`.

## Build & Test Commands

```bash
cargo build                              # debug build
cargo test                               # run Rust tests (src/tests)
cargo test <test_name>                   # run a specific Rust test
cargo test <test_name> -- --nocapture    # run with stdout visible
sapphire test [path]                     # run Sapphire tests (_test.spr files)
sapphire test ./stdlib/tests             # run tests in a specific directory
./scripts/ci                             # run all CI checks (cargo test, clippy, sapphire test, examples)
```

## Testing Framework

Sapphire includes two testing approaches:

**Rust integration tests** in `tests/` run the VM through the Rust API. Common patterns:
- `eval(src)` — compile and run source, return `VmValue`
- `eval_with_stdlib(src)` — same but with stdlib loaded
- `eval_err(src)` — assert that code raises a `VmError`

**Sapphire tests** are Sapphire files ending in `_test.spr` that extend the `Test` class (from `stdlib/src/test.spr`). Tests are discovered and run recursively with `sapphire test [path]`, which outputs:
- `.` for passing tests, `F` for failures
- Summary line showing test count and timing (tests/sec)

Test classes inherit from `Test` and provide assertion methods:
- `assert(cond)` — fails if condition is falsy
- `assert_equal(expected, actual)` — fails with formatted message on mismatch
- `assert_nil(obj)` — fails unless value is nil
- `assert_in_delta(expected, actual, delta)` — fuzzy float comparison
- `assert_raises { block }` — fails unless block raises an exception

## Architecture

There is one execution pipeline: **Lexer → Parser → Compiler → VM**

All execution paths (`run`, `test`, `console`, and the REPL loop) go through this same pipeline. There is no tree-walk interpreter.

Key files:
- `src/main.rs` — CLI entry point; routes to `run_file`, `typecheck_file`, `run_tests`, or `run_repl`
- `src/lexer.rs` — Tokenizes source into `Token`s
- `src/token.rs` — Token type definitions
- `src/parser.rs` — Recursive descent parser producing an AST; `call()` handles method/field access and auto-call behavior
- `src/ast.rs` — All AST node definitions (`Expr` enum, `MethodDef`, `Block`, etc.)
- `src/compiler.rs` — Compiles AST to bytecode; `compile()` for scripts, `compile_repl()` for REPL
- `src/chunk.rs` — `Chunk` (bytecode + constants), `OpCode` enum, `Function`, `UpvalueDef`
- `src/vm.rs` — Stack-based bytecode VM (`Vm::run`); defines `VmValue` (the runtime value type)
- `src/value.rs` — `Value` enum: primitive constants only (`Int`, `Float`, `Bool`, `Str`, `Nil`) used in the compiler/chunk layer
- `src/typechecker.rs` — Optional static type checker (two-pass: collect definitions, then check bodies); invoked only by `typecheck` subcommand
- `src/error.rs` — Error types
- `stdlib/` — Standard library written in Sapphire itself, embedded in the binary and loaded by `vm.load_stdlib()`

## Runtime Value System

`VmValue` (defined in `src/vm.rs`) is the runtime representation of all values. Key variants:
- Primitives: `Int(i64)`, `Float(f64)`, `Bool(bool)`, `Str(String)`, `Nil`
- Collections: `List`, `Map`, `Range { from, to }`
- Callables: `Closure`, `NativeFunction`, `NativeMethod`, `Block`
- OOP: `Class { name, fields, methods, class_methods, namespace, superclass }`, `Instance { class_name, fields, methods }`, `BoundMethod`

`Instance` stores `fields` in a `Rc<RefCell<HashMap>>` and `methods` in a plain `HashMap`. `Class` stores instance methods, class methods (from `self { }` blocks), and nested classes (in `namespace`).

`value.rs` is a separate, simpler `Value` enum used only for compile-time constants embedded in `Chunk`.

## Key Compiler Patterns

- `Call { callee: Get { object, name }, args }` compiles to `OpCode::Invoke(name, arg_count)` — the common fast path for method dispatch
- `Call { callee: Variable(name), args }` compiles to `OpCode::Call(arg_count)` after pushing the callee
- Zero-arg method calls don't require parentheses: `obj.foo` and `obj.foo()` are equivalent (both parse to `Expr::Call` wrapping `Expr::Get`)
- `def foo { }` and `def foo() { }` are equivalent (parentheses optional on zero-arg definitions)
- `Expr::Get` (bare field access without call) is only emitted when used as an lvalue or in specific non-call contexts
- **`Foo.new(args)` always compiles to `OpCode::NewInstance`**, not `Invoke("new", …)` — special-cased in `src/compiler.rs`. The class-method dispatch chain in `vm.rs` is never reached for `new`. To intercept construction of a new value type, add a guard at the top of the `OpCode::NewInstance` handler, not in the class-method chain.

## Language Design Constraints

- No global variables, class variables, or metaprogramming
- Top-level `def` desugars into `Object` methods (Ruby-style)
- Primitives (`Int`, `Float`, `Str`, etc.) are objects with methods via stdlib classes
- Single inheritance; `defp` for private methods; `self { }` block for class methods
- Gradual typing: type annotations on parameters/return types are optional but enforced at runtime when present
- Imports are relative paths (`./` or `../`), `.spr` extension added automatically; each file executes once

## Standard Library

Stdlib files in `stdlib/` are embedded as string literals and loaded during `vm.load_stdlib()`. Each file adds methods to a primitive type's class (`int.spr`, `float.spr`, `string.spr`, `bool.spr`, `nil.spr`, `list.spr`, `map.spr`) plus `object.spr` for the base `Object` class.

See `docs/adding-stdlib-class.md` for a step-by-step guide to adding a new stdlib class backed by native Rust dispatch.
