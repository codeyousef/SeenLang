# Core Types

## Option\<T\>

Generic optional value container. Represents a value that may or may not exist.

```seen
import core.option
```

### Constructors

```seen
let some = Some(42)          // Option with value
let none = None<Int>()       // Empty option
let opt = Option<Int>.New()  // Empty option (alternative)
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `isSome()` | `Bool` | True if value exists |
| `isNone()` | `Bool` | True if empty |
| `unwrap()` | `T` | Get value (panics if empty) |
| `expect(message: String)` | `T` | Get value (panics with message if empty) |
| `unwrapOr(default: T)` | `T` | Get value or return default |
| `Store(value: T)` | `Void` | Set the value |
| `Replace(value: T)` | `Option<T>` | Replace value, return old |
| `Clear()` | `Void` | Remove value |

### Example

```seen
fun findUser(id: Int) r: Option<String> {
    if id == 1 {
        return Some("Alice")
    }
    return None<String>()
}

fun main() {
    let user = findUser(1)
    if user.isSome() {
        println("Found: {user.unwrap()}")
    }
    let name = findUser(99).unwrapOr("Unknown")
}
```

## Result\<T, E\>

Container for success/error values.

```seen
import core.result
```

### Constructors

```seen
let ok = Ok<Int, String>(42)
let err = Err<Int, String>("not found")
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `isOkay()` | `Bool` | True if success |
| `isErr()` | `Bool` | True if error |
| `unwrap()` | `T` | Get success value (panics if error) |
| `expect(message: String)` | `T` | Get value (panics with message if error) |
| `unwrapOr(default: T)` | `T` | Get value or return default |
| `unwrapErr()` | `E` | Get error value (panics if success) |
| `swap()` | `Result<E, T>` | Swap Ok and Err types |

### The `?` Operator

Propagates errors:

```seen
fun process() r: Result<Int, String> {
    let x = parse("42")?   // returns Err early if parse fails
    return Ok(x * 2)
}
```

### Example

```seen
fun divide(a: Int, b: Int) r: Result<Int, String> {
    if b == 0 {
        return Err("division by zero")
    }
    return Ok(a / b)
}

fun main() {
    let result = divide(10, 3)
    if result.isOkay() {
        println("Result: {result.unwrap()}")
    } else {
        println("Error: {result.unwrapErr()}")
    }
}
```

## Unit

The unit type, used as a placeholder when no value is needed:

```seen
import core.unit

let u = unit()
```

Useful as the value type in `Result<Unit, E>` or map types like `HashMap<String, Unit>` (set semantics).

## Ordering

Comparison result type:

```seen
import core.ord

enum Ordering {
    Less
    Equal
    Greater
}
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `compareInt` | `(a: Int, b: Int) r: Ordering` | Compare two integers |
| `compareString` | `(a: String, b: String) r: Ordering` | Lexicographic string comparison |
| `isLess` | `(ord: Ordering) r: Bool` | Check if Less |
| `isEqual` | `(ord: Ordering) r: Bool` | Check if Equal |
| `isGreater` | `(ord: Ordering) r: Bool` | Check if Greater |

## Type Conversion

```seen
import core.convert
```

### Int conversions

| Function | Signature |
|----------|-----------|
| `Int_from_Float` | `(value: Float) r: Int` |
| `Int_from_String` | `(value: String) r: Int` |
| `Int_from_Bool` | `(value: Int) r: Int` |

### Float conversions

| Function | Signature |
|----------|-----------|
| `Float_from_Int` | `(value: Int) r: Float` |
| `Float_from_String` | `(value: String) r: Float` |

### String conversions

| Function | Signature |
|----------|-----------|
| `String_from_Int` | `(value: Int) r: String` |
| `String_from_Float` | `(value: Float) r: String` |
| `String_from_Bool` | `(value: Int) r: String` |

### Casting with `as`

```seen
let x: Int = 42
let f = x as Float     // 42.0
let s = x as String    // "42"
```

## ToString Trait

```seen
@trait class ToString {
    fun toString() r: String
}
```

Free function converters:
- `Int_toString(value: Int) r: String`
- `Float_toString(value: Float) r: String`
- `Bool_toString(value: Int) r: String`
- `Char_toString(value: Int) r: String`
