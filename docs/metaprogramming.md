# Metaprogramming

Seen supports compile-time evaluation, decorators, derive macros, and reflection.

## Decorators

Decorators are annotations that modify code generation. They use the `@` prefix.

### Function Decorators

| Decorator | Effect |
|-----------|--------|
| `@async` | Transform function into LLVM coroutine |
| `@inline` / `@always_inline` | Force inlining |
| `@intrinsic` | Map to LLVM intrinsic |
| `@compute(workgroup_size=N)` | GPU compute shader |
| `@test` | Mark as test function |
| `@benchmark` | Mark as benchmark function |
| `@init` | Run at module initialization |

### Class Decorators

| Decorator | Effect |
|-----------|--------|
| `@derive(...)` | Generate trait implementations |
| `@reflect` | Enable runtime type information |
| `@packed` | Remove field padding |
| `@cache_line` | Align to cache line |
| `@trivially_copyable` | Allow memcpy |
| `@move` | Enforce move semantics |
| `@nondeterministic` | Allow in deterministic mode |
| `@component` | Register in component framework |
| `@store` | Mutation-tracking state store |

### Modifier Decorators

| Decorator | Effect |
|-----------|--------|
| `@send` | Type safe to transfer between threads |
| `@sync` | Type safe to share between threads |
| `@cfg("feature")` | Conditional compilation |
| `@repr(C)` | C-compatible struct layout |

## Derive Macros

`@derive` auto-generates trait implementations:

```seen
@derive(Clone, Hash, Eq, Debug)
class Point {
    var x: Float
    var y: Float
}
```

### Available Derives

| Derive | Generated |
|--------|-----------|
| `Clone` | `clone()` -- deep copy |
| `Hash` | `hash()` -- hash code |
| `Eq` | `eq(other)` -- equality check |
| `Debug` | `debug()` -- debug string representation |
| `Serialize` | Binary serialization |
| `Deserialize` | Binary deserialization |
| `Json` | JSON serialization/deserialization |

### Example: JSON Derive

```seen
@derive(Json)
class Config {
    var name: String
    var port: Int
    var debug: Bool
}

fun main() {
    let config = Config { name: "app", port: 8080, debug: true }
    let json = config.toJson()
    println(json)  // {"name":"app","port":8080,"debug":true}
}
```

## Reflection

`@reflect` enables runtime type information (RTTI):

```seen
@reflect
class User {
    var name: String
    var age: Int
    var email: String
}

fun main() {
    let user = User { name: "Alice", age: 30, email: "alice@example.com" }

    // Get field names at runtime
    let fields = user.fieldNames()
    for field in fields {
        println(field)  // "name", "age", "email"
    }

    // Check if field exists
    if reflectHasField(user.fieldNames(), "email") {
        println("User has email field")
    }
}
```

## Compile-Time Evaluation

### comptime functions

Functions evaluated at compile time:

```seen
comptime fun factorial(n: Int) r: Int {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}

let TABLE_SIZE = comptime { factorial(10) }
```

### comptime blocks

Arbitrary compile-time computation:

```seen
let LOOKUP = comptime {
    var table = Array<Int>.withLength(256)
    var i = 0
    while i < 256 {
        table[i] = i * i
        i = i + 1
    }
    table
}
```

### comptime if

Conditional compilation based on compile-time values:

```seen
comptime if TARGET == "wasm" {
    fun allocate(size: Int) r: Int {
        return wasm_alloc(size)
    }
} else {
    fun allocate(size: Int) r: Int {
        return malloc(size)
    }
}
```

## @intrinsic

Map a Seen function directly to an LLVM intrinsic:

```seen
@intrinsic("llvm.sqrt.f64")
extern fun __Sqrt(x: Float) r: Float

@intrinsic("llvm.sin.f64")
extern fun __Sin(x: Float) r: Float

@intrinsic("llvm.fabs.f64")
extern fun __Fabs(x: Float) r: Float
```

## Feature Flags

Conditional compilation via feature flags:

```bash
seen build app.seen --feature=gpu --feature=experimental
```

In source code:

```seen
@cfg("gpu")
fun renderWithGPU() {
    // only compiled when --feature=gpu is passed
}

@cfg("experimental")
class ExperimentalOptimizer {
    // only compiled when --feature=experimental is passed
}
```

## Macros

Basic macro support for code generation:

```seen
macro define_getter(field_name, field_type) {
    fun get_{field_name}() r: {field_type} {
        return this.{field_name}
    }
}
```

## Related

- [Language Guide](language-guide.md) -- syntax fundamentals
- [API Reference: Reflect](api-reference/reflect.md) -- reflection API
- [API Reference: JSON](api-reference/json.md) -- JSON derive
- [API Reference: Binary](api-reference/binary.md) -- serialization
