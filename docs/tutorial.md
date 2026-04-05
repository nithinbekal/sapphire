# Sapphire Tutorial

Sapphire is an object-oriented scripting language with a Ruby-inspired feel: clean syntax, everything is an object, and blocks make iteration expressive. This tutorial walks through the language from the ground up.

## Running Sapphire

```
sapphire run hello.spr   # run a script file
sapphire                 # start the interactive REPL
```

File extension: `.spr`

---

## Variables

Variables are assigned with `=`. No declaration keyword is needed.

```
x = 10
name = "Alice"
flag = true
```

Variable names can contain letters, digits, and underscores, and may end with `?`.

```
empty? = false
```

---

## Types

Sapphire has five built-in value types:

| Type   | Examples                    |
|--------|-----------------------------|
| Int    | `0`, `42`, `-7`             |
| Bool   | `true`, `false`             |
| Str    | `"hello"`                   |
| Array  | `[1, 2, 3]`                 |
| Nil    | `nil`                       |

---

## Arithmetic

```
1 + 2       # 3
10 - 3      # 7
4 * 5       # 20
10 / 3      # 3  (integer division)
-x          # negation
```

Operator precedence follows the usual rules. Use parentheses to override.

```
(1 + 2) * 3   # 9
```

---

## Comparisons

```
x == 10
x != 5
x < 10
x > 0
x <= 10
x >= 1
```

These return `true` or `false`.

---

## Logical operators

```
x > 0 && x < 100   # both must be true
x == 0 || x == 1   # at least one must be true
!flag               # negation
```

`&&` and `||` short-circuit: the right side is not evaluated if the result is already determined.

```
name = nil
label = name || "unknown"   # "unknown"
```

---

## Strings

String literals use double quotes.

```
greeting = "hello"
```

### Concatenation

```
"hello" + " " + "world"   # "hello world"
```

### Interpolation

Embed any expression inside `#{}`:

```
name = "Alice"
age = 30
print "Hello #{name}, you are #{age} years old!"
# Hello Alice, you are 30 years old!
```

Any value is automatically converted to a string in interpolation.

```
nums = [1, 2, 3]
print "Array: #{nums}"   # Array: [1, 2, 3]
```

### Type conversions

```
42.to_s       # "42"
"123".to_i    # 123
true.to_s     # "true"
```

---

## Comments

Comments begin with `#` and run to the end of the line.

```
# This is a comment
x = 10   # inline comment
```

---

## Print

```
print "Hello, world!"
print x
print x + 1
```

---

## Control flow

### if / else

```
if x > 0 {
  print "positive"
} else {
  print "non-positive"
}
```

`else` is optional.

```
if flag {
  print "yes"
}
```

A single-statement body can be written on one line:

```
if x < 0 { print "negative" }
```

### while

```
i = 0
while i < 5 {
  print i
  i = i + 1
}
```

---

## Functions

Define functions with `def`. The last evaluated expression is the implicit return value. `return` is optional, but useful for early exits.

```
def add(a, b) {
  a + b
}

print add(3, 4)   # 7
```

### Explicit return

```
def abs(n) {
  if n < 0 { return -n }
  n
}
```

### Closures

Functions close over the variables in scope where they are defined.

```
def make_adder(n) {
  def adder(x) {
    x + n
  }
  adder
}

add5 = make_adder(5)
print add5(3)   # 8
```

### Predicates

Function names (and variables) may end with `?` to signal a boolean result.

```
def zero?(x) {
  x == 0
}

print zero?(0)   # true
print zero?(1)   # false
```

---

## Arrays

Array literals use square brackets.

```
nums = [1, 2, 3, 4, 5]
empty = []
mixed = [1, "hello", true]
```

### Index access

Zero-based. Negative indices count from the end.

```
nums[0]    # 1
nums[-1]   # 5
```

### Index assignment

```
nums[0] = 99
```

### Built-in methods

```
nums.length   # 5
nums.first    # 1
nums.last     # 5
nums.push(6)  # appends 6, returns 6
nums.pop()    # removes and returns the last element
```

---

## Blocks

Blocks are anonymous chunks of code passed to a method call. They use `{ |param| body }` syntax.

```
nums = [1, 2, 3, 4, 5]

nums.each { |n| print n }
```

### map

Returns a new array with each element transformed.

```
doubled = nums.map { |n| n * 2 }
print doubled   # [2, 4, 6, 8, 10]
```

### select

Returns a new array with only the elements for which the block returns `true`.

```
evens = nums.select { |n| n % 2 == 0 }
print evens   # [2, 4]
```

### Blocks can mutate outer variables

```
sum = 0
nums.each { |n| sum = sum + n }
print sum   # 15
```

---

## nil

`nil` represents the absence of a value.

```
x = nil
print x   # nil
```

### nil?

```
x = nil
print x.nil?    # true

y = 42
print y.nil?    # false
```

### Safe navigation (&.)

Use `&.` to call a method on a value that might be `nil`. If the receiver is `nil`, the whole expression evaluates to `nil` instead of raising an error.

```
user = nil
print user&.name   # nil

user = User.new(name: "Bob")
print user&.name   # Bob
```

---

## User input

`read_line()` reads a line from standard input and returns it as a string.

```
print "What is your name?"
name = read_line()
print "Hello, #{name}!"
```

To use the input as a number, convert it with `to_i`:

```
print "Enter a number:"
n = read_line().to_i
print n * 2
```

---

## Classes

Define a class with `class`. Use `attr` to declare fields. Instantiate with `ClassName.new(field: value, ...)`.

```
class Point {
  attr x
  attr y
}

p = Point.new(x: 3, y: 4)
print p.x   # 3
print p.y   # 4
```

### Default field values

```
class Circle {
  attr radius
  attr color = "red"
}

c = Circle.new(radius: 5)
print c.color   # red
```

### Methods

Define methods with `def` inside the class body. Use `self` to refer to the current instance.

```
class Point {
  attr x
  attr y

  def distance_from_origin() {
    dx = self.x * self.x
    dy = self.y * self.y
    dx + dy   # returns dx^2 + dy^2 (no sqrt yet)
  }

  def to_s() {
    "(#{self.x}, #{self.y})"
  }
}

p = Point.new(x: 3, y: 4)
print p.distance_from_origin()   # 25
print p.to_s()                   # (3, 4)
```

### Mutating fields

Assign to `self.field` inside a method to update an instance field.

```
class Counter {
  attr count

  def increment() {
    self.count = self.count + 1
  }

  def reset() {
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

---

## Inheritance

Use `class Child < Parent` to inherit from a parent class. The subclass gets all of the parent's fields, and can override methods.

```
class Animal {
  attr name

  def speak() {
    print "..."
  }

  def greet() {
    print "I am #{self.name}"
  }
}

class Dog < Animal {
  def speak() {
    print "Woof!"
  }
}

class Cat < Animal {
  def speak() {
    print "Meow!"
  }
}

d = Dog.new(name: "Rex")
c = Cat.new(name: "Whiskers")

d.speak()    # Woof!
d.greet()    # I am Rex  (inherited from Animal)
c.speak()    # Meow!
```

### Adding new fields in a subclass

```
class Vehicle {
  attr make
  attr model
}

class ElectricVehicle < Vehicle {
  attr range_km

  def describe() {
    print "#{self.make} #{self.model}, range: #{self.range_km}km"
  }
}

ev = ElectricVehicle.new(make: "Tesla", model: "Model 3", range_km: 500)
ev.describe()   # Tesla Model 3, range: 500km
```

---

## Putting it together

Here is a small program that uses most of the language:

```
class TodoList {
  attr items

  def add(item) {
    self.items.push(item)
  }

  def done_count() {
    count = 0
    self.items.each { |item| count = count + 1 }
    count
  }

  def print_all() {
    i = 0
    self.items.each { |item|
      i = i + 1
      print "#{i}. #{item}"
    }
  }
}

list = TodoList.new(items: [])
list.add("Buy groceries")
list.add("Write tests")
list.add("Ship it")

list.print_all()
# 1. Buy groceries
# 2. Write tests
# 3. Ship it

print "Total: #{list.done_count()}"
# Total: 3
```
