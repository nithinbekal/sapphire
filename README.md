# Sapphire

Sapphire is a statically typed, object-oriented programming language built as a learning project to explore interpreter and compiler design.

It follows Ruby's object model — everything is an object, interfaces act as mixins — but with a cleaner, more explicit syntax: static types, no sigils, no metaprogramming, and parens always required on method calls.

## Goals

- Explore Ruby's object model in a statically typed setting
- Experiment with OOP features: classes, inheritance, interfaces as mixins, and closures
- Follow the [Crafting Interpreters](https://craftinginterpreters.com) path — tree-walk interpreter first, bytecode VM later
- Keep the language small and the implementation readable

## Design Principles

- **Everything is an object** — primitives like `Int` and `Bool` are objects with methods
- **Interfaces as mixins** — interfaces can be mixed into classes, not just used as type constraints
- **Static types** — no duck typing; types are explicit and checked at compile time
- **No magic** — no global variables, no class variables (`@@`), no metaprogramming

## Syntax

### Classes

```sapphire
class Point {
  attr x: Int
  attr y: Int
  attr label: String = "Foo"

  def to_s : Str { "<Point #{x} #{y}>" }
}
```

### Instantiation

```sapphire
let p = Point(x: 1, y: 2)
```

### Inheritance

```sapphire
class Point3D < Point {
  attr z: Int
}
```

### Interfaces

```sapphire
interface Printable {
  def to_s : Str
}
```

## Status

Very early — currently building the lexer.

## Implementation

Written in Rust.
