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

Sapphire has six built-in value types:

| Type   | Examples                          |
|--------|-----------------------------------|
| Int    | `0`, `42`, `-7`                   |
| Bool   | `true`, `false`                   |
| Str    | `"hello"`                         |
| List   | `[1, 2, 3]`                       |
| Map    | `{ name: "Alice", age: 30 }`      |
| Nil    | `nil`                             |

---

## Arithmetic

```
1 + 2       # 3
10 - 3      # 7
4 * 5       # 20
10 / 3      # 3  (integer division)
10 % 3      # 1  (modulo)
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

### Escape sequences

```
"hello\nworld"   # newline
"tab\there"      # tab
"quote: \""      # literal quote
"backslash: \\"  # literal backslash
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
print "List: #{nums}"   # List: [1, 2, 3]
```

### String methods

```
"hello".length          # 5
"hello".upcase          # "HELLO"
"HELLO".downcase        # "hello"
"  hi  ".strip          # "hi"
"hello".empty?          # false
"".empty?               # true

"hello".include?("ell")       # true
"hello".starts_with?("hel")   # true
"hello".ends_with?("llo")     # true

"a,b,c".split(",")      # ["a", "b", "c"]
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

### break

`break` exits the enclosing loop or block iteration immediately. When used with a value, that value is returned from the iterator.

```
i = 0
while true {
  i = i + 1
  break if i == 3
}
print i   # 3
```

```
result = [1, 2, 3, 4, 5].each { |x|
  break "found it" if x == 3
}
print result   # found it
```

### next

`next` skips the rest of the current iteration and moves to the next one.

```
i = 0
sum = 0
while i < 5 {
  i = i + 1
  next if i == 3
  sum = sum + i
}
print sum   # 12  (1+2+4+5, skipped 3)
```

Inside a `map` block, `next val` sets the value for that element instead of computing the rest of the block:

```
result = [1, 2, 3, 4].map { |x|
  next 0 if x == 2
  x * 10
}
print result   # [10, 0, 30, 40]
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

## Lists

List literals use square brackets.

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
nums.push(6)  # appends 6
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

Returns a new list with each element transformed.

```
doubled = nums.map { |n| n * 2 }
print doubled   # [2, 4, 6, 8, 10]
```

### select

Returns a new list with only the elements for which the block returns `true`.

```
evens = nums.select { |n| n % 2 == 0 }
print evens   # [2, 4]
```

### reduce

Folds a list into a single value. Pass an initial accumulator, or omit it to use the first element.

```
sum = nums.reduce(0) { |acc, n| acc + n }
print sum   # 15

product = nums.reduce { |acc, n| acc * n }
print product   # 120
```

### any? / all? / none?

```
nums = [1, 2, 3, 4, 5]

nums.any? { |n| n > 4 }    # true
nums.all? { |n| n > 0 }    # true
nums.none? { |n| n > 9 }   # true
```

### Blocks can mutate outer variables

```
sum = 0
nums.each { |n| sum = sum + n }
print sum   # 15
```

---

## Integer iteration

`.times` runs a block `n` times, passing the current index (starting at 0).

```
3.times { |i| print i }
# 0
# 1
# 2
```

---

## Maps

Map literals use `{ key: value }` syntax. Keys are always strings.

```
person = { name: "Alice", age: 30 }
```

### Access and mutation

```
person["name"]          # "Alice"
person["city"] = "Dublin"
```

### Built-in methods

```
person.length           # 3
person.keys             # ["age", "city", "name"]
person.values           # [30, "Dublin", "Alice"]
person.has_key?("name") # true
```

### Iterating

```
person.each { |k, v| print "#{k}: #{v}" }
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

## Trailing conditionals

A statement can be conditionally executed by appending `if condition` at the end.

```
print "negative" if x < 0
return nil if name.nil?
x = x * 2 if x > 0
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

Define methods with `def` inside the class body. Fields and other methods are accessible by name — no `self.` prefix needed for reads.

```
class Point {
  attr x
  attr y

  def distance_from_origin() {
    dx = x * x
    dy = y * y
    dx + dy   # returns dx^2 + dy^2 (no sqrt yet)
  }

  def to_s() {
    "(#{x}, #{y})"
  }
}

p = Point.new(x: 3, y: 4)
print p.distance_from_origin()   # 25
print p.to_s()                   # (3, 4)
```

### Mutating fields

Reading a field uses the bare name. Writing to a field requires `self.field =`.

```
class Counter {
  attr count

  def increment() {
    self.count = count + 1
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
    print "I am #{name}"
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
    print "#{make} #{model}, range: #{range_km}km"
  }
}

ev = ElectricVehicle.new(make: "Tesla", model: "Model 3", range_km: 500)
ev.describe()   # Tesla Model 3, range: 500km
```

### super

Use `super.method_name(args)` inside a method to call the same or a different method from the parent class. `self` is passed through automatically.

```
class Animal {
  attr name

  def describe() {
    name
  }
}

class Dog < Animal {
  attr breed

  def describe() {
    super.describe() + " (" + breed + ")"
  }
}

d = Dog.new(name: "Rex", breed: "Lab")
print d.describe()   # Rex (Lab)
```

`super` always operates on the same `self` instance, so field changes in the subclass are visible to the superclass method and vice versa.

---

## yield

`yield` calls the block that was passed to the current method. This lets you write iterators and higher-order methods in Sapphire itself.

```
def call_twice() {
  yield(1)
  yield(2)
}

call_twice() { |n| print n }
# 1
# 2
```

`yield(args)` passes arguments to the block and returns the block's value.

```
def transform(x) {
  yield(x)
}

result = transform(5) { |n| n * 10 }
print result   # 50
```

### Writing iterators in Sapphire

```
def my_each(list) {
  len = list.length
  i = 0
  while i < len {
    yield(list[i])
    i = i + 1
  }
}

sum = 0
my_each([1, 2, 3]) { |x| sum = sum + x }
print sum   # 6
```

`yield` works inside methods too:

```
class NumberList {
  attr items

  def each() {
    len = items.length
    i = 0
    while i < len {
      yield(items[i])
      i = i + 1
    }
  }
}

nums = NumberList.new(items: [10, 20, 30])
total = 0
nums.each() { |x| total = total + x }
print total   # 60
```

---

## Putting it together

Here is a small program that uses most of the language:

```
class TodoList {
  attr items

  def add(item) {
    items.push(item)
  }

  def count() {
    items.reduce(0) { |acc, item| acc + 1 }
  }

  def print_all() {
    i = 0
    items.each { |item|
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

print "Total: #{list.count()}"
# Total: 3
```
