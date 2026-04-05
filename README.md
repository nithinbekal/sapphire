# Sapphire

Sapphire is an object-oriented programming language built as a learning project to explore interpreter and compiler design.

It follows Ruby's object model — everything is an object, interfaces act as mixins — but with a cleaner, more explicit syntax: no sigils on variables, no metaprogramming, and parens always required on method calls.

## Goals

- Explore Ruby's object model in a typed setting
- Experiment with OOP features: classes, inheritance, interfaces as mixins, and closures
- Follow the [Crafting Interpreters](https://craftinginterpreters.com) path — tree-walk interpreter first, bytecode VM later
- Keep the language small and the implementation readable

## Design Principles

- **Everything is an object** — primitives like `Int` and `Bool` are objects with methods
- **Interfaces as mixins** — interfaces can be mixed into classes, not just used as type constraints
- **No magic** — no global variables, no class variables, no metaprogramming

## Running

```
sapphire run file.spr   # run a file
sapphire               # start the REPL
```

## Syntax

### Variables

```sapphire
x = 10
name = "alice"
flag = true
```

### Arithmetic and comparisons

```sapphire
1 + 2 * 3
x == 10
x > 0
!flag
```

### Control flow

```sapphire
if x > 0 {
  print x
} else {
  print 0
}

while x < 10 {
  x = x + 1
}
```

### Functions

```sapphire
def add(a, b) {
  a + b
}

def abs(n) {
  if n < 0 { return -n }
  n
}

add(1, 2)
```

### Classes

```sapphire
class Point {
  attr x: Int
  attr y: Int
  attr label: Str = "origin"
}

p = Point.new(x: 1, y: 2)
p.x      # => 1
p.label  # => "origin"
```

## Planned

- Instance methods
- Inheritance (`class Point3D < Point`)
- Interfaces as mixins
- Static types enforced at compile time
- Bytecode VM
