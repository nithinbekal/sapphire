# Changelog

## v0.2.0

**VM**
- The bytecode VM is now the sole runtime — the tree-walk interpreter has been removed
- The REPL (`sapphire console`) now runs on the VM

**Parser fixes**

Method chaining after a block now works, both on one line and across lines:

```ruby
# Now works — previously: parse error: unexpected token 'Dot'
[1, 2, 3].map { |n| n * 2 }.each { |n| print n }

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
