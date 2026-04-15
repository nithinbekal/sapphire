# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Sapphire?

Sapphire is an object-oriented scripting language implemented in Rust. It's Ruby-inspired with gradual typing. The CLI has four subcommands: `run`, `typecheck`, `console` (REPL), and `version`.

## Build & Test Commands

```bash
cargo build                              # debug build
cargo test                               # run all tests
cargo test <test_name>                   # run a specific test
cargo test <test_name> -- --nocapture    # run with stdout visible
```

## Architecture

There is one execution pipeline: **Lexer → Parser → Compiler → VM**

All three entry points (`run`, `console`, and the REPL loop) go through this same path. There is no tree-walk interpreter.

Key files:
- `src/main.rs` — CLI entry point; routes to `run_file`, `typecheck_file`, or `run_repl`
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

## Language Design Constraints

- No global variables, class variables, or metaprogramming
- Top-level `def` desugars into `Object` methods (Ruby-style)
- Primitives (`Int`, `Float`, `Str`, etc.) are objects with methods via stdlib classes
- Single inheritance; `defp` for private methods; `self { }` block for class methods
- Gradual typing: type annotations on parameters/return types are optional but enforced at runtime when present
- Imports are relative paths (`./` or `../`), `.spr` extension added automatically; each file executes once

## Standard Library

Stdlib files in `stdlib/` are embedded as string literals and loaded during `vm.load_stdlib()`. Each file adds methods to a primitive type's class (`int.spr`, `float.spr`, `string.spr`, `bool.spr`, `nil.spr`, `list.spr`, `map.spr`) plus `object.spr` for the base `Object` class.
