# Sapphire Tutorial

Sapphire is an object-oriented scripting language with a Ruby-inspired feel: clean syntax, everything is an object, and blocks make iteration expressive. This tutorial walks through the language from the ground up.

## Running Sapphire

```sh
sapphire run hello.spr   # run a script file
sapphire console         # start the interactive REPL
```

File extension: `.spr`

---

## The basics

Variables are assigned with `=`. No declaration keyword is needed. String literals use double quotes, and you can embed any expression inside `#{}`:

```ruby
name = "Alice"
age = 30
pi = 3.14

print "Hello, #{name}!"           # Hello, Alice!
print "Next year: #{age + 1}"     # Next year: 31
print "Pi is about #{pi}"         # Pi is about 3.14
```

Comments begin with `#` and run to the end of the line. The `print` built-in accepts any value and writes it to standard output.

Sapphire has six built-in value types:

| Type   | Examples                          |
|--------|-----------------------------------|
| Int    | `0`, `42`, `-7`                   |
| Float  | `3.14`, `-0.5`                    |
| Bool   | `true`, `false`                   |
| Str    | `"hello"`                         |
| List   | `[1, 2, 3]`                       |
| Map    | `{ name: "Alice", age: 30 }`      |
| Nil    | `nil`                             |

Variable names contain letters, digits, and underscores. The `?` suffix is reserved for method names — not variables.

---

## Control flow

### if / elsif / else

```ruby
score = 72

if score >= 90 {
  print "A"
} elsif score >= 80 {
  print "B"
} elsif score >= 70 {
  print "C"
} else {
  print "below C"
}
# C
```

A trailing `if` is a concise way to guard a single statement:

```ruby
print "passing" if score >= 60
```

### while, break, and next

```ruby
i = 1
sum = 0

while i <= 10 {
  next if i % 2 == 0   # skip even numbers
  sum = sum + i
  break if sum > 15    # stop once we've accumulated enough
  i = i + 1
}

print sum   # 16  (1+3+5+7)
```

`next` skips to the next iteration; `break` exits the loop entirely.

### Ranges

A range literal `from..to` represents an inclusive sequence of integers. Use `.each` to iterate or `.include?` to test membership:

```ruby
(1..5).each { |i| print i }
# 1  2  3  4  5

(1..100).include?(42)   # true
```

---

## Functions

Define functions with `def`. The last evaluated expression is the implicit return value — no explicit `return` needed except for early exits.

```ruby
def greet(name) {
  "Hello, #{name}!"
}

print greet("world")   # Hello, world!
```

### Gradual typing

Type annotations on parameters and return values are optional. When present, they are enforced at runtime — that's gradual typing. Add them where you want safety; leave them off when flexibility matters more.

```ruby
# Without annotations — works for any value
def double(x) {
  x * 2
}

# With annotations — enforced at runtime
def double(x: Int) -> Int {
  x * 2
}

print double(5)     # 10
print double(2.5)   # runtime error: expected Int
```

Annotate as much or as little as you like. A common pattern is to annotate public functions at module boundaries and leave internal helpers unannotated.

### Early return

`return` is useful for exiting a function before reaching the end:

```ruby
def abs(n: Int) -> Int {
  return -n if n < 0
  n
}
```

### Predicates

By convention, functions that return a boolean end with `?`:

```ruby
def even?(n: Int) -> Bool {
  n % 2 == 0
}

print even?(4)   # true
print even?(7)   # false
```

### Closures

Functions close over variables in scope where they are defined:

```ruby
def make_adder(n) {
  def adder(x) {
    x + n
  }
  adder
}

add5 = make_adder(5)
print add5(3)   # 8
print add5(10)  # 15
```

### yield

`yield` calls the block passed to the current method, letting you write your own iterators:

```ruby
def repeat(n: Int) {
  i = 0
  while i < n {
    yield(i)
    i = i + 1
  }
}

repeat(3) { |i| print "step #{i}" }
# step 0
# step 1
# step 2
```

---

## Collections

### Lists

List literals use square brackets. Indexing is zero-based; negative indices count from the end.

```ruby
nums = [3, 1, 4, 1, 5, 9, 2, 6]

print nums[0]    # 3
print nums[-1]   # 6
print nums.size   # 8

nums.append(7)
nums.pop()       # remove and return last
```

The core iteration methods let you work with lists without manual loops:

```ruby
nums = [3, 1, 4, 1, 5, 9, 2, 6]

doubled  = nums.map    { |n| n * 2 }
big      = nums.select { |n| n > 4 }
total    = nums.reduce(0) { |acc, n| acc + n }

print doubled   # [6, 2, 8, 2, 10, 18, 4, 12]
print big       # [5, 9, 6]
print total     # 31

print nums.any? { |n| n > 8 }   # true   (9 is)
print nums.all? { |n| n > 0 }   # true
print nums.none? { |n| n > 99 } # true
```

Blocks can also read and write outer variables:

```ruby
sum = 0
nums.each { |n| sum = sum + n }
print sum   # 31
```

### Maps

Map literals use `{ key: value }` syntax. Keys are always strings.

```ruby
person = { name: "Alice", age: 30 }

print person["name"]   # Alice

person["city"] = "Dublin"

print person.keys      # ["name", "age", "city"]
print person.size    # 3

person.each { |k, v| print "#{k}: #{v}" }
```

---

## Classes

### Fields and methods

Define a class with `class`. Use `attr` to declare fields. Instantiate with `ClassName.new(field: value, ...)`.

Inside a method body, fields and other methods are accessible by their bare name. `self.` is only required when *writing* to a field.

```ruby
class Point {
  attr x: Int
  attr y: Int

  def distance_sq -> Int {
    x * x + y * y
  }

  def to_s -> Str {
    "(#{x}, #{y})"
  }
}

p = Point.new(x: 3, y: 4)
print p.to_s()            # (3, 4)
print p.distance_sq()     # 25
```

Fields can have default values:

```ruby
class Circle {
  attr radius: Int
  attr color = "red"
}

c = Circle.new(radius: 5)
print c.color   # red
```

### Mutating fields

Reading uses the bare field name; writing requires `self.field =`:

```ruby
class Counter {
  attr count = 0

  def increment {
    self.count = count + 1
  }

  def reset {
    self.count = 0
  }
}

c = Counter.new(count: 0)
c.increment()
c.increment()
c.increment()
print c.count   # 3
c.reset()
print c.count   # 0
```

### Private methods

`defp` declares a private method — callable from within the class but not from outside:

```ruby
class BankAccount {
  attr balance = 0

  def deposit(amount: Int) {
    self.balance = balance + validate(amount)
  }

  defp validate(amount: Int) -> Int {
    raise "amount must be positive" if amount <= 0
    amount
  }
}

acc = BankAccount.new(balance: 0)
acc.deposit(100)
print acc.balance   # 100

acc.validate(50)    # error: private method
```

### Class methods

A `self { ... }` block inside the class body defines methods called on the class itself. Use them for factory methods and class-level behaviour:

```ruby
class Point {
  attr x: Int
  attr y: Int

  self {
    def origin {
      self.new(x: 0, y: 0)
    }
  }
}

p = Point.origin()
print p.x   # 0
```

### Inheritance

Use `class Child < Parent` to inherit from a parent class. Subclasses inherit all fields and methods, and can override methods:

```ruby
class Animal {
  attr name: Str

  def speak {
    print "..."
  }

  def greet {
    print "I am #{name}"
  }
}

class Dog < Animal {
  def speak {
    print "Woof!"
  }
}

class Cat < Animal {
  def speak {
    print "Meow!"
  }
}

d = Dog.new(name: "Rex")
c = Cat.new(name: "Whiskers")

d.speak()   # Woof!
d.greet()   # I am Rex  (inherited from Animal)
c.speak()   # Meow!
```

Use bare `super` or `super(...)` to call the superclass method with the same name (like Ruby):

```ruby
class Animal {
  attr name: Str

  def describe -> Str {
    name
  }
}

class Dog < Animal {
  attr breed: Str

  def describe -> Str {
    super() + " (#{breed})"
  }
}

d = Dog.new(name: "Rex", breed: "Lab")
print d.describe()   # Rex (Lab)
```

Every class implicitly inherits from `Object`. `is_a?(ClassName)` checks the class hierarchy:

```ruby
d = Dog.new(name: "Rex", breed: "Lab")
d.is_a?(Dog)      # true
d.is_a?(Animal)   # true
d.is_a?(Object)   # true
d.is_a?(Cat)      # false
```

---

## Error handling

Use `raise` to signal an error. Handle errors with a `begin / rescue / end` block:

```ruby
def parse_age(s: Str) -> Int {
  age = s.to_i
  raise "age must be positive" if age <= 0
  age
}

begin
  print parse_age("25")   # 25
  print parse_age("0")    # raises
rescue e
  print "bad input: #{e}"
end
```

The `else` clause runs only when no error occurred:

```ruby
begin
  result = 10 / 2
rescue e
  print "error: #{e}"
else
  print "result: #{result}"   # result: 5
end
```

A `rescue` clause inside a `def` body avoids the `begin / end` wrapper entirely:

```ruby
def safe_divide(a: Int, b: Int) -> Int {
  a / b
rescue e
  print "cannot divide by zero"
  0
}

print safe_divide(10, 2)   # 5
print safe_divide(10, 0)   # 0
```

You can raise any object, not just strings:

```ruby
class AppError {
  attr message: Str
}

begin
  raise AppError.new(message: "not found")
rescue e
  print e.message   # not found
end
```

---

## Imports

Split your code across files with `import`. The path must be relative (starting with `./` or `../`) and the `.spr` extension is added automatically:

```ruby
import "./utils"
import "../shared/helpers"
```

Everything defined at the top level of the imported file — classes and functions — becomes available in the importing file. Each file is only executed once, even if imported from multiple places.

A common pattern is a library file that defines a class:

```ruby
# geometry/point.spr
class Point {
  attr x: Float
  attr y: Float

  def distance_to(other) -> Float {
    dx = self.x - other.x
    dy = self.y - other.y
    ((dx * dx) + (dy * dy)).sqrt()
  }

  def to_s -> Str {
    "(#{self.x}, #{self.y})"
  }
}
```

And a main file that uses it:

```ruby
# main.spr
import "./geometry/point"

a = Point.new(x: 0.0, y: 0.0)
b = Point.new(x: 3.0, y: 4.0)

print a.to_s()              # (0.0, 4.0)
print a.distance_to(b)      # 5.0
```

Files can import other files — a file inside `geometry/` can itself import from the same directory using `./`:

```ruby
# geometry/shapes.spr
import "./point"

class Circle {
  attr center: Point
  attr radius: Float

  def area -> Float {
    3.14159 * self.radius * self.radius
  }
}
```

---

## Putting it together

Here is a small program that uses classes, inheritance, type annotations, blocks, and error handling. It models geometric shapes and computes statistics over a collection of them.

```ruby
class Shape {
  def area -> Float {
    raise "area() not implemented"
  }

  def describe -> Str {
    "Shape with area #{self.area()}"
  }
}

class Rectangle < Shape {
  attr width: Float
  attr height: Float

  def area -> Float {
    width * height
  }

  def describe -> Str {
    "Rectangle #{width}x#{height}, area=#{self.area()}"
  }
}

class Circle < Shape {
  attr radius: Float

  def area -> Float {
    3.14159 * radius * radius
  }

  def describe -> Str {
    "Circle r=#{radius}, area=#{self.area()}"
  }
}

def summarise(shapes) {
  shapes.each { |s| print s.describe() }

  total = shapes.reduce(0.0) { |acc, s| acc + s.area() }
  largest = shapes.reduce(shapes[0]) { |best, s|
    if s.area() > best.area() { s } else { best }
  }

  print "Total area:   #{total}"
  print "Largest:      #{largest.describe()}"
}

shapes = [
  Rectangle.new(width: 4.0, height: 5.0),
  Circle.new(radius: 3.0),
  Rectangle.new(width: 2.0, height: 8.0),
]

begin
  summarise(shapes)
rescue e
  print "Error: #{e}"
end
```

Output:

```
Rectangle 4.0x5.0, area=20.0
Circle r=3.0, area=28.27431
Rectangle 2.0x8.0, area=16.0
Total area:   64.27431
Largest:      Circle r=3.0, area=28.27431
```
