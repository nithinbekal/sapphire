# Changelog

## v0.5.2

**CI**

- Reverted parallel WASM build — the native and WASM release jobs now run sequentially again

---

## v0.5.1

This is a version bump to verify the release pipeline.

**CI**

- WASM build now runs in parallel with native builds, reducing total release time

---

## v0.5.0

**Language**

- Heredoc strings — triple-quoted multi-line string literals with automatic indent stripping:

```ruby
message = """
  Hello,
  world!
  """
```

- `return` now works correctly inside blocks passed to native methods such as `each`

**Standard library**

- `DateTime` module — `Instant`, `Date`, `Time`, `DateTime`, `ZonedDateTime`, and `Duration` types for date and time handling
- `Math` — trigonometric methods: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`
- `Set` — unordered collection with set-membership semantics
- `Socket` — minimal TCP client support via `Socket.new` and `connect`, `send`, `receive`, `close`
- `Env` — read and write environment variables with `Env.get`, `Env.set`, and `Env.all`
- `Process` — run subprocesses with `Process.run`; result is a `Process.Result` with `stdout`, `stderr`, and `exit_code`
- `Class#instance_method_names` — returns a list of method names defined on a class
- All collection types now consistently use `size` instead of `length`

**CLI**

- `sapphire test` now reports the total test run time

**Bug fixes**

- `Map#all?` now handles entries with `nil` values correctly
- `Map#none?` no longer recurses infinitely

---

## v0.4.0

**Language**

- Class-level constants — define named constants directly inside a class body:

```ruby
class Circle {
  PI = 3.14159

  def area(r) { PI * r * r }
}
```

- Bitwise operators — `&`, `|`, `^`, `~`, `<<`, `>>` are now supported on integers
- Numeric literal improvements — underscore separators and hexadecimal literals are now valid:

```ruby
population = 8_000_000_000
color      = 0xFF5733
```

- Parentheses are optional for zero-argument method definitions and calls:

```ruby
def greet { "hello" }   # same as def greet() { "hello" }

greet                    # same as greet()
"hello".upcase           # same as "hello".upcase()
```

**Standard library**

- `Math` class with `Math.PI` and `Math.E` constants
- `File` class — `File.read(path)`, `File.write(path, content)`, and `File.exist?(path)` for basic file I/O

**CLI**

- `sapphire test` — built-in test runner for `.spr` test files

**Editor support**

- Vim plugin — syntax highlighting for `.spr` files is available at [sapphire-project/vim-sapphire](https://github.com/sapphire-project/vim-sapphire)

**REPL**

- Command history and multiline input support in `sapphire console`

**VM**

- Mark-and-sweep garbage collector to break reference cycles in object graphs

**Bug fixes**

- Class namespace constants defined inside nested classes are now correctly preserved when loading the standard library

---

## v0.3.0

**Language**

- Nested class definitions — classes can now be defined inside other classes and accessed with dot notation (`Geometry.Point`), including as superclasses:

```ruby
class Geometry {
  class Point {
    attr x: 0
    attr y: 0
  }
}

p = Geometry.Point.new(x: 1, y: 2)
```

- Relative file imports — use `import "./path"` to load a `.spr` file relative to the current file; imported classes and functions become available in the importing file; duplicate imports are silently skipped

**VM**

- Return type annotations are now enforced at runtime — functions declared with `-> TypeName` raise a type error if the return value doesn't match; the `Num` supertype accepts both `Int` and `Float`

**Bug fixes**

- `break` inside blocks passed to native methods (e.g. `each`, `map`, `select`) now works correctly — previously it would silently stop execution past the native call
- `break` and `next` inside `while` loops now work correctly

---

## v0.2.1

**Bug fixes**
- `Float#to_s` now preserves the trailing `.0` for whole-number floats (`1.0.to_s()` returns `"1.0"` instead of `"1"`)
- `Float#zero?` now returns `true` for `0.0` (previously always returned `false` due to an integer comparison)

---

## v0.2.0

**VM**
- The bytecode VM is now the sole runtime — the tree-walk interpreter has been removed
- The REPL (`sapphire console`) now runs on the VM

**Parser fixes**

Method chaining after a block now works, both on one line and across lines:

```ruby
# Now works — previously: parse error: unexpected token 'Dot'
[1, 2, 3].map { |n| n * 2 }.each { |n| print n }

# Also works now
[1, 2, 3]
  .map { |n| n * 2 }
  .each { |n| print n }
```

`elsif` and `else` can now appear on the next line after the closing `}`:

```ruby
# Now works — previously: parse error: unexpected token 'Elsif'
if x == 1 { "one" }
elsif x == 2 { "two" }
else { "other" }
```

---

## v0.1.1

**Classes**
- Class methods via `self { ... }` blocks — methods callable on the class itself, inherited by subclasses

**CLI**
- `sapphire version` command — prints the language name and version (e.g. `Sapphire 0.1.1`)
- More detailed usage output

---

## v0.1.0

First public preview of the Sapphire language.

### Language features

**Primitives & literals**
- `Int`, `Float`, `Bool`, `Nil` literals
- String literals with interpolation (`"hello #{name}"`) and escape sequences (`\n`, `\t`, `\\`, `\"`)
- Range literals (`1..5`)
- List literals (`[1, 2, 3]`) with index access and mutation
- Map literals (`{x: 1, y: 2}`) with string key access and mutation

**Operators**
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Boolean: `&&`, `||`, `!`
- String concatenation with `+`
- Safe navigation: `obj&.method`
- Modulo with division-by-zero error handling

**Variables & assignment**
- Variable assignment and reassignment
- Multiple assignment (`a, b = 1, 2`)
- Swap (`a, b = b, a`)

**Control flow**
- `if` / `elsif` / `else` — as a statement or expression
- `while` loops
- Postfix / trailing `if` (`raise "msg" if condition`)
- `return` for early exit

**Functions & closures**
- Named functions with `def`
- Closures that capture variables from enclosing scopes
- First-class anonymous lambdas (`f = def(x) { x * 2 }; f.call(5)`)
- Top-level `def` desugars into `Object` methods (Ruby-style)

**Blocks**
- Block syntax: `list.each() { |x| print x }`
- `yield` to call a passed block
- `next` to return a value from the current block iteration
- `break` to exit the block's caller early

**Classes**
- Class definitions with `attr` fields and default values
- Keyword constructor: `Point.new(x: 1, y: 2)`
- Instance methods with implicit `self`
- Private methods with `defp`
- Single inheritance (`class Dog < Animal`)
- `super` for calling parent methods
- `is_a?` with full inheritance chain check

**Error handling**
- `raise` with a string message
- `begin` / `rescue` / `else` / `end` blocks
- Inline rescue inside `def` bodies
- `begin`/`rescue` as an expression

**Standard library**
- `Int`: `to_s`, `to_f`, `abs`, `even?`, `odd?`, `zero?`, `times`
- `Float`: `round`, `floor`, `ceil`, `to_i`, `abs`
- `String`: `size`, `upcase`, `downcase`, `reverse`, `strip`, `to_i`, `to_f`, `empty?`, `include?`, `starts_with?`, `ends_with?`, `split`
- `Bool` / `Nil`: `nil?`, `to_s`
- `List`: `size`, `first`, `last`, `empty?`, `include?`, `sort`, `join`, `push`, `pop`, `each`, `map`, `select`, `any?`, `all?`, `none?`, `reduce`, `flatten`
- `Map`: `size`, `has_key?`, `delete`, `merge`, `each`, `select`, `any?`, `all?`, `none?`
- `Range`: `each`, `to_a`, `include?`
- `Object`: `is_a?`, `nil?`, `class`

**Type system**
- Optional type annotations on parameters and return types
- Runtime enforcement when annotations are present
- Static type checker (`sapphire typecheck`)

**Execution**
- Tree-walk interpreter (`sapphire run`)
- Bytecode compiler + stack-based VM (`sapphire vm`)
- Interactive REPL (`sapphire console`)
