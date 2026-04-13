# Sapphire

Sapphire is an object-oriented programming language built as a learning project to explore interpreter and compiler design.

It follows Ruby's object model — everything is an object — but adds gradual typing and simplifies the syntax where possible.

## Goals

- Explore Ruby's object model in a typed setting
- Experiment with OOP features: classes, inheritance, and closures
- Follow the [Crafting Interpreters](https://craftinginterpreters.com) path — tree-walk interpreter first, bytecode VM later
- Keep the language small and the implementation readable

## Design Principles

- **Everything is an object** — primitives like `Int` and `Bool` are objects with methods
- **No magic** — no global variables, no class variables, no metaprogramming

## Running

```
sapphire run file.spr   # run a file
sapphire                # start the REPL
```

## Syntax

### Variables

```ruby
x = 10
name = "alice"
flag = true
```

### Arithmetic and comparisons

```ruby
1 + 2 * 3
x == 10
x > 0
!flag
```

### Control flow

```ruby
if x > 0 {
  print x
} elsif x == 0 {
  print "zero"
} else {
  print "negative"
}

while x < 10 {
  x = x + 1
}

(1..5).each { |i| print i }
```

### Functions

```ruby
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

```ruby
class Point {
  attr x
  attr y
  attr label = "origin"
}

p = Point.new(x: 1, y: 2)
p.x      # => 1
p.label  # => "origin"
```

Use `defp` for private methods:

```ruby
class BankAccount {
  attr balance

  def deposit(amount) { self.balance = balance + validate(amount) }

  defp validate(amount) {
    raise "must be positive" if amount <= 0
    amount
  }
}
```

All objects inherit from `Object` and respond to `is_a?`:

```ruby
p.is_a?(Point)    # true
p.is_a?(Object)   # true
```

### Error handling

```ruby
begin
  result = risky_op()
rescue e
  print "caught: #{e}"
else
  print "ok: #{result}"
end
```

Inline rescue inside a `def`:

```ruby
def safe_div(a, b) {
  a / b
rescue e
  0
}
```

## Current Limitations

This is an early preview. Known gaps:

- **No imports** — all code must live in a single file
- **No class methods** — only instance methods are supported
- **No garbage collection** — reference-counted memory; cycles will leak
- **REPL** — no command history or multiline input
