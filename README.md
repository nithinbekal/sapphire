# Sapphire

A Ruby-inspired, gradually typed, object-oriented scripting language — everything is an object, types are optional, and the syntax stays out of your way.

**[Website](https://sapphire-lang.dev/)** · **[Try it online](https://sapphire-lang.dev/try/)** · **[Tutorial](https://sapphire-lang.dev/tutorial/)**

## Features

- **Gradual typing** — annotate as much or as little as you like; types are checked at runtime when present
- **Everything is an object** — `Int`, `Bool`, `String`, and other primitives have methods
- **Classes with inheritance** — single inheritance, `attr` fields, private methods via `defp`, class methods
- **Closures and blocks** — first-class functions, `yield`, and block-accepting methods
- **Rich standard library** — `List`, `Map`, `Set`, `String`, `Regex`, `Math`, `Date`, `File`, and more
- **Imports** — split code across files with `import`
- **Mark-and-sweep GC** — handles cycles; no manual memory management

## Quick look

```ruby
class Shape {
  attr color = "red"

  def area { 0 }

  def describe {
    "A #{self.color} shape with area #{self.area()}"
  }
}

class Circle < Shape {
  attr radius: Float

  def area {
    Math::PI * self.radius * self.radius
  }
}

c = Circle.new(color: "blue", radius: 3.0)
print c.describe()
print c.is_a?(Shape)   # true
```

## Syntax

### Variables and types

```ruby
x = 10
name: String = "alice"
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
def add(a: Int, b: Int) -> Int {
  a + b
}

def clamp(value: Int, min: Int, max: Int) -> Int {
  return min if value < min
  return max if value > max
  value
}
```

Blocks and `yield`:

```ruby
def repeat(n: Int) {
  i = 0
  while i < n {
    yield(i)
    i = i + 1
  }
}

repeat(3) { |i| print "step #{i}" }
```

### Classes

```ruby
class BankAccount {
  attr balance: Int = 0

  def deposit(amount: Int) {
    self.balance = self.balance + validate(amount)
  }

  def withdraw(amount: Int) {
    self.balance = self.balance - validate(amount)
  }

  defp validate(amount: Int) -> Int {
    raise "amount must be positive" if amount <= 0
    amount
  }
}

account = BankAccount.new()
account.deposit(100)
account.withdraw(30)
print account.balance   # 70
```

Class methods use `self { }`:

```ruby
class Color {
  attr r: Int
  attr g: Int
  attr b: Int

  self {
    def red   { Color.new(r: 255, g: 0, b: 0) }
    def green { Color.new(r: 0, g: 255, b: 0) }
    def blue  { Color.new(r: 0, g: 0, b: 255) }
  }
}

c = Color.red()
```

### Collections

```ruby
numbers = [3, 1, 4, 1, 5, 9]

doubled = numbers.map { |n| n * 2 }
evens   = numbers.select { |n| n % 2 == 0 }
total   = numbers.reduce(0) { |acc, n| acc + n }

print numbers.any? { |n| n > 8 }   # true
print numbers.all? { |n| n > 0 }   # true
```

```ruby
scores = { alice: 95, bob: 82, carol: 91 }
scores.each { |name, score| print "#{name}: #{score}" }
passing = scores.select { |_, score| score >= 90 }
```

### Imports

```ruby
import "./geometry/point"
import "./geometry/shapes"

p = Point.new(x: 1.0, y: 2.0)
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

Inline rescue inside a function:

```ruby
def safe_div(a: Int, b: Int) -> Int {
  a / b
rescue e
  0
}
```

## Running

```
sapphire run file.spr      # run a file
sapphire test              # run *_test.spr files
sapphire typecheck file.spr
sapphire console           # start the REPL
```
