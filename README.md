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

All objects inherit from `Object` and respond to `is_a?` and `respond_to?`:

```ruby
p.is_a?("Point")    # true
p.is_a?("Object")   # true
p.respond_to?("to_s")  # true
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
