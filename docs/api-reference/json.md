# JSON

## @derive(Json)

Auto-generate JSON serialization and deserialization:

```seen
@derive(Json)
class User {
    var name: String
    var age: Int
    var active: Bool
}

fun main() {
    let user = User { name: "Alice", age: 30, active: true }
    let json = user.toJson()
    println(json)
    // {"name":"Alice","age":30,"active":true}
}
```

## JSON Type Constants

```seen
import core.json_derive
```

| Constant | Value | Description |
|----------|-------|-------------|
| `JSON_NULL` | 0 | Null value |
| `JSON_BOOL` | 1 | Boolean |
| `JSON_INT` | 2 | Integer |
| `JSON_FLOAT` | 3 | Float |
| `JSON_STRING` | 4 | String |
| `JSON_ARRAY` | 5 | Array |
| `JSON_OBJECT` | 6 | Object |

## JSON Module

```seen
import json
```

### JSONValue

The `JSONValue` class represents any JSON value.

### JSON Parser

Parse JSON strings:

```seen
import json.parser

let value = parseJson(jsonString)
```

### JSON Builder

Build JSON programmatically:

```seen
import json.builder
```

## Runtime JSON Functions

| Function | Description |
|----------|-------------|
| `seen_json_parse(str)` | Parse JSON string |
| `seen_json_stringify(val)` | Serialize to JSON string |
| `seen_json_get_type(val)` | Get value type |
| `seen_json_get_int(val)` | Extract integer |
| `seen_json_get_float(val)` | Extract float |
| `seen_json_get_string(val)` | Extract string |
| `seen_json_get_bool(val)` | Extract boolean |
| `seen_json_array_length(val)` | Get array length |
| `seen_json_array_get(val, idx)` | Get array element |
| `seen_json_object_get(val, key)` | Get object field |

## Example

```seen
@derive(Json)
class Config {
    var host: String
    var port: Int
    var debug: Bool
    var tags: Array<String>
}

fun main() {
    let config = Config {
        host: "localhost",
        port: 8080,
        debug: false,
        tags: ["web", "api"]
    }

    let json = config.toJson()
    println(json)
    // {"host":"localhost","port":8080,"debug":false,"tags":["web","api"]}
}
```
