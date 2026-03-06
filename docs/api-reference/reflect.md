# Reflection

## @reflect

Enable runtime type information (RTTI) for a class:

```seen
@reflect
class Entity {
    var id: Int
    var name: String
    var health: Float
}
```

The `@reflect` decorator generates metadata that can be queried at runtime.

## API

```seen
import core.reflect
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `reflectHasField` | `(fieldNames: Array<String>, name: String) r: Bool` | Check if field exists |
| `reflectFieldIndex` | `(fieldNames: Array<String>, name: String) r: Int` | Get field index (-1 if not found) |

## Usage

```seen
@reflect
class Player {
    var name: String
    var score: Int
    var level: Int
}

fun main() {
    let player = Player { name: "Alice", score: 100, level: 5 }

    // Get field names
    let fields = player.fieldNames()
    for field in fields {
        println("Field: {field}")
    }
    // Output:
    //   Field: name
    //   Field: score
    //   Field: level

    // Check field existence
    if reflectHasField(fields, "score") {
        let idx = reflectFieldIndex(fields, "score")
        println("score is at index {idx}")
    }
}
```

## Combining with @derive

Reflection works well with derive macros:

```seen
@reflect
@derive(Debug, Clone, Json)
class Config {
    var name: String
    var port: Int
    var debug: Bool
}

fun main() {
    let config = Config { name: "app", port: 8080, debug: true }

    // Reflection
    let fields = config.fieldNames()
    println("Fields: {fields.length()}")

    // Debug (from @derive(Debug))
    println(config.debug())

    // JSON (from @derive(Json))
    println(config.toJson())
}
```

## Limitations

- Reflection only provides field names, not field types at runtime
- Only works on classes decorated with `@reflect`
- Field access is by index, not by name (use `reflectFieldIndex` to map)
