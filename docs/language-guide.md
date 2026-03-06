# Language Guide

This is the reference for Seen's syntax and semantics.

## Variables

### Immutable (`let`)

```seen
let name = "Alice"
let age = 30
let pi = 3.14159
```

### Mutable (`var`)

```seen
var count = 0
count = count + 1
```

### Value binding (`val`)

```seen
val MAX_SIZE = 1024
```

### Constants and statics

```seen
const PI = 3.14159265
static var instance_count = 0
```

## Type Annotations

Types can be explicit or inferred:

```seen
let x: Int = 42
let y: Float = 3.14
let s: String = "hello"
let b: Bool = true
let c: Char = 'A'
```

### Built-in Types

| Type | Description |
|------|-------------|
| `Int` | 64-bit signed integer |
| `Float` | 64-bit floating point |
| `Bool` | Boolean (true/false) |
| `String` | UTF-8 string |
| `Char` | Unicode code point |
| `Void` | No value |
| `Never` | Function never returns |

## Functions

### Basic function

```seen
fun greet(name: String) r: String {
    return "Hello, {name}!"
}
```

The `r:` syntax specifies the return type.

### Void functions (no return)

```seen
fun logMessage(msg: String) {
    println(msg)
}
```

### Fat arrow syntax

For single-expression bodies:

```seen
fun double(x: Int) r: Int => x * 2
```

### Multiple parameters

```seen
fun add(a: Int, b: Int) r: Int {
    return a + b
}
```

### Default parameters are not yet supported — use overloads or optional types.

## Control Flow

### if / else

```seen
if x > 0 {
    println("positive")
} else {
    println("non-positive")
}
```

### if as expression

```seen
let result = if x > 0 { "positive" } else { "non-positive" }
```

### while loop

```seen
var i = 0
while i < 10 {
    println("{i}")
    i = i + 1
}
```

### for-in loop

```seen
for item in items {
    println("{item}")
}
```

### Range-based for

```seen
for i in 0..10 {
    println("{i}")  // 0 through 9
}

for i in 0..=10 {
    println("{i}")  // 0 through 10
}
```

### loop (infinite)

```seen
loop {
    let input = readLine()
    if input == "quit" {
        break
    }
}
```

### break and continue

```seen
for i in 0..100 {
    if i == 50 { break }
    if i % 2 == 0 { continue }
    println("{i}")
}
```

## Pattern Matching

### when expression

```seen
let result = when value {
    is 1 => "one"
    is 2 => "two"
    is 3 => "three"
    else => "other"
}
```

### match on enums

```seen
when shape {
    is Circle(r) => println("Circle with radius {r}")
    is Rectangle(w, h) => println("Rectangle {w}x{h}")
}
```

## Classes

### Basic class

```seen
class Point {
    var x: Float
    var y: Float

    static fun new(x: Float, y: Float) r: Point {
        return Point { x: x, y: y }
    }

    fun distanceTo(other: Point) r: Float {
        let dx = this.x - other.x
        let dy = this.y - other.y
        return sqrt(dx * dx + dy * dy)
    }
}
```

### Constructors

Use `static fun new(...)` by convention:

```seen
let p = Point.new(3.0, 4.0)
```

### `this` keyword

Instance methods access fields via `this`:

```seen
fun getX() r: Float {
    return this.x
}
```

### Inheritance

```seen
class Shape {
    var name: String

    fun describe() r: String {
        return "Shape: {this.name}"
    }
}

class Circle extends Shape {
    var radius: Float

    fun area() r: Float {
        return 3.14159 * this.radius * this.radius
    }
}
```

## Structs

### Data struct (value type)

```seen
data struct Color(r: Int, g: Int, b: Int)
```

This is a compact value type declaration.

### Regular struct (alias for class)

```seen
struct Config {
    var width: Int
    var height: Int
    var title: String
}
```

## Enums

### Simple enum

```seen
enum Direction {
    North
    South
    East
    West
}
```

### Data-carrying enum

```seen
enum Shape {
    Circle(radius: Float)
    Rectangle(width: Float, height: Float)
    Triangle(base: Float, height: Float)
}
```

### Using enums

```seen
let dir = Direction.North
let shape = Shape.Circle(5.0)
```

## Traits and impl

### Defining a trait

```seen
trait Printable {
    fun display() r: String
}
```

### Implementing a trait

```seen
impl Printable for Point {
    fun display() r: String {
        return "({this.x}, {this.y})"
    }
}
```

### Trait constraints on generics

```seen
fun printItem<T: Printable>(item: T) {
    println(item.display())
}
```

## Generics

### Generic functions

```seen
fun max<T>(a: T, b: T) r: T {
    if a > b { return a }
    return b
}
```

### Generic classes

```seen
class Stack<T> {
    var items: Array<T>

    static fun new() r: Stack<T> {
        return Stack { items: Array<T>() }
    }

    fun push(item: T) {
        this.items.push(item)
    }

    fun pop() r: T {
        return this.items.pop()
    }

    fun isEmpty() r: Bool {
        return this.items.length() == 0
    }
}
```

### Generic constraints

```seen
fun sort<T: Ord>(arr: Array<T>) r: Array<T> {
    // T must implement Ord
}
```

## Closures

```seen
let doubled = apply(nums, |x| x * 2)
let filtered = items.filter(|x| x > 0)
```

Closures use `|params| expression` syntax. Currently no-capture (function pointer) semantics.

## Nullable Types

### Optional types with `T?`

```seen
var name: String? = null
name = "Alice"
```

### Safe access with `?.`

```seen
let len = name?.length()  // returns null if name is null
```

### Null coalescing with `??`

```seen
let displayName = name ?? "Anonymous"
```

### if-let pattern

```seen
if let n = name {
    println("Name is {n}")
}
```

## Result Types

### Result<T, E>

```seen
fun divide(a: Int, b: Int) r: Result<Int, String> {
    if b == 0 {
        return Err("division by zero")
    }
    return Ok(a / b)
}
```

### The `?` operator

Propagates errors to the caller:

```seen
fun compute() r: Result<Int, String> {
    let x = divide(10, 2)?
    let y = divide(x, 3)?
    return Ok(x + y)
}
```

### try / catch

```seen
try {
    let result = riskyOperation()
    println("Success: {result}")
} catch e {
    println("Error: {e}")
}
```

## String Interpolation

Use `{expression}` inside double-quoted strings:

```seen
let name = "World"
println("Hello, {name}!")

let x = 42
println("The answer is {x}")

let p = Point.new(3.0, 4.0)
println("Distance: {p.distanceTo(Point.new(0.0, 0.0))}")
```

## Arrays and Collections

### Array literals

```seen
let nums = [1, 2, 3, 4, 5]
let names = ["Alice", "Bob", "Charlie"]
```

### Array<T> operations

```seen
var arr = Array<Int>()
arr.push(1)
arr.push(2)
let first = arr.get(0)
let len = arr.length()
```

### Array with initial size

```seen
let zeros = Array<Int>.withLength(100)
```

### HashMap

```seen
var map = HashMap<String, Int>()
map.insert("alice", 30)
map.insert("bob", 25)
let age = map.get("alice")
```

See the [Collections API](api-reference/collections.md) for Vec, BTreeMap, LinkedList, and more.

## Ranges

```seen
0..10      // exclusive: 0, 1, 2, ..., 9
0..=10     // inclusive: 0, 1, 2, ..., 10
```

## Operators

### Arithmetic

`+`, `-`, `*`, `/`, `%`

### Comparison

`==`, `!=`, `<`, `<=`, `>`, `>=`

### Logical

`and`, `or`, `not` (also `&&`, `||`, `!`)

### Bitwise

`&`, `|`, `^`, `~`, `<<`, `>>`

### Compound assignment

`+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`

## Type Casting

Use `as` for type conversions:

```seen
let x: Int = 42
let f = x as Float
let s = x as String
```

## Modules and Imports

### Import

```seen
import io
import collections.HashMap
```

### Visibility

`pub` makes declarations visible outside the module:

```seen
pub fun publicFunction() {
    // accessible from other modules
}

fun privateFunction() {
    // only accessible within this module
}
```

## Operator Overloading

```seen
class Vec2 {
    var x: Float
    var y: Float

    operator fun +(other: Vec2) r: Vec2 {
        return Vec2.new(this.x + other.x, this.y + other.y)
    }

    operator fun *(scalar: Float) r: Vec2 {
        return Vec2.new(this.x * scalar, this.y * scalar)
    }
}
```

## Unsafe Blocks

For low-level operations that bypass safety checks:

```seen
unsafe {
    let ptr = transmute(address)
    // raw pointer operations
}
```

## Defer

Execute cleanup code when leaving a scope:

```seen
fun processFile(path: String) {
    let file = File.open(path)
    defer { file.close() }

    // file is automatically closed when scope exits
    let content = file.readContent()
}
```

### errdefer

Only executes if the scope exits via error:

```seen
fun allocateResource() r: Result<Resource, String> {
    let res = acquire()
    errdefer { release(res) }

    let configured = configure(res)?  // if this fails, res is released
    return Ok(configured)
}
```

## Type Aliases

```seen
type Callback = Fun
type StringList = Array<String>
```

### Distinct types

```seen
distinct Meters = Float
distinct Seconds = Float

// Meters and Seconds are incompatible even though both are Float
```

## Extension Methods

```seen
extension fun String.isBlank() r: Bool {
    return trim(this) == ""
}
```

## Next Steps

- [CLI Reference](cli-reference.md) -- compiler commands and flags
- [Memory Model](memory-model.md) -- ownership, regions, borrowing
- [API Reference](api-reference/index.md) -- standard library
