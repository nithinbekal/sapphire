# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Sapphire?

Sapphire is an object-oriented programming language implemented as a tree-walk interpreter in Rust. It's Ruby-inspired with gradual typing. The CLI has three subcommands: `run`, `typecheck`, and `console` (REPL).

## Build & Test Commands

```bash
cargo build                          # debug build
cargo test                           # run all tests
cargo test <test_name>               # run a specific test
cargo test <test_name> -- --nocapture  # run with stdout
```

## Architecture

The interpreter follows a classic pipeline:

**Lexer → Parser → (TypeChecker) → Interpreter**

Key files:
- `src/main.rs` — CLI entry point; routes to `run_file`, `typecheck_file`, or `run_repl`
- `src/lexer.rs` — Tokenizes source into `Token`s
- `src/parser.rs` — Recursive descent parser producing an AST
- `src/ast.rs` — AST node definitions
- `src/interpreter.rs` — Tree-walk interpreter; `execute()` for statements, `evaluate()` for expressions
- `src/typechecker.rs` — Optional static type checker (two-pass: collect defs, then check bodies)
- `src/value.rs` — `Value` enum (the runtime representation of all values)
- `src/environment.rs` — Lexically-scoped variable bindings (parent-child chain of hashmaps)
- `src/error.rs` — Error types
- `stdlib/` — Standard library written in Sapphire itself, embedded in the binary at init

## Runtime Value System

`Value` is the central enum. Key variants:
- Primitives: `Int(i64)`, `Float(f64)`, `Bool(bool)`, `Str(String)`, `Nil`
- Collections: `List(Rc<RefCell<Vec<Value>>>)`, `Map(...)`, `Range { from, to }`
- Callables: `Class`, `Constructor`, `Instance`, `BoundMethod`, `NativeFunction`, `NativeMethod`, `Block`

## Language Design Constraints

- No global variables, class variables, or metaprogramming
- Top-level `def` desugars into `Object` methods (Ruby-style)
- Primitives (`Int`, `Float`, `String`, etc.) are objects with methods via stdlib classes
- Single inheritance; interfaces work as mixins
- Gradual typing: type annotations on parameters/return types are optional but enforced at runtime when present

## Standard Library

Stdlib files in `stdlib/` are embedded as string literals and loaded during `global_env()` initialization. Each file adds methods to a primitive type's class (`int.spr`, `float.spr`, `string.spr`, `bool.spr`, `nil.spr`, `list.spr`, `map.spr`) plus `object.spr` for the base `Object` class.
