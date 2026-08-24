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
| `JSON_NUMBER` | 2 | Integer or floating-point number |
| `JSON_STRING` | 3 | String |
| `JSON_ARRAY` | 4 | Array |
| `JSON_OBJECT` | 5 | Object |

## JSON Module

```seen
import json
```

### JsonValue

The `JsonValue` class represents any JSON value. Constructors include
`JsonValue_nullValue`, `JsonValue_fromBool`, `JsonValue_bool`,
`JsonValue_number`, `JsonValue_numberFloat`, `JsonValue_string`,
`JsonValue_stringCopy`, `JsonValue_array`, `JsonValue_emptyArray`,
`JsonValue_object`, and `JsonValue_emptyObject`.

### JSON Parser

Parse JSON strings and release the complete result tree exactly once:

```seen
import json.{parseJson, destroyJsonParseResult}

let result = parseJson(jsonString)
if result.success == 1 {
    let root = result.value.unwrap()
    // Read root while result remains alive.
    println(root.length().toString())
}
destroyJsonParseResult(result, true)
```

`JsonParser` and `JsonParseResult` expose parser state and success/error
results for callers that need more control than a one-shot parse helper. A
successful parse result owns its root. Pass `true` to
`destroyJsonParseResult` to destroy the tree with the result, or unwrap the
root and pass `false` to transfer that root to the caller. After a transfer,
the caller must eventually call `JsonValue_destroy(root)`.

`get` and `getAt` return a newly allocated `Option` wrapper around a borrowed
child. Free the wrapper after inspecting it. The child remains owned by its
root and must not be destroyed separately. `set` and `push` move a detached
child into a container exactly once. Reusing an attached child or inserting a
container into itself is rejected as a no-op; the container remains unchanged.
`getArray` and `keys` retain their shipped backing-handle behavior: treat the
returned array as borrowed and read-only, and do not free it. Structural
changes go through `push` and `set`.
The array and object constructors fail closed if their input repeats an
already attached child. They shallow-copy their input array containers: the
caller retains those input arrays and may mutate or free them after the call,
while each contained `JsonValue` moves into the returned JSON tree exactly
once. Object-key strings remain borrowed. `JsonValue_string` borrows its input string;
`JsonValue_stringCopy` creates an independently owned copy. The parser also
keeps explicit ownership of the copies it creates for parsed strings and
object keys.

`JsonValue_destroy` preflights the complete tree before freeing anything. It
aborts with a stable ownership diagnostic for duplicate/cyclic value handles,
detached children, invalid kinds, or inconsistent object geometry. Direct
mutation or freeing of representation fields/backing arrays remains
unsupported. JSON values are thread-confined: move a detached root between
threads only with external synchronization; do not concurrently read, mutate,
or destroy one tree.

The one-shot parser defaults to at most 64 MiB of UTF-8 input, 256 nested
containers, and one million total JSON values. Security-sensitive callers may
choose smaller positive limits:

```seen
import json.{parseJsonWithLimits, destroyJsonParseResult}

let result = parseJsonWithLimits(payload, 1048576, 64, 100000)
if result.success == 0 {
    println(result.error)
}
destroyJsonParseResult(result, true)
```

Input, nesting, and value-limit failures are deterministic parse errors. The
limits bound recursion and graph growth; partial arrays, objects, strings, and
result wrappers are released on every error path. Number conversion uses a
runtime range parser, avoiding a full-number substring allocation.

### JSON Builder

Build JSON programmatically:

```seen
import json.builder
```

`JsonBuilder` and `JsonBuilder_new` provide incremental JSON construction.
The builder stores output in `StringBuilder` parts internally, so large objects
and arrays grow linearly instead of repeatedly copying the whole JSON string.
`escapeJsonString`, `jsonToString`, `toJsonPretty`, and `JsonBuilder.build`
each return an independent owned string. Call `releaseJsonText` exactly once
after the result is no longer needed. Call `destroyJsonBuilder` exactly once
for an incremental builder; it releases escaped-string parts retained between
`build` calls. `reset` performs the same part cleanup while keeping the builder
available for reuse.

```seen
import json.{jsonToString, releaseJsonText}

let text = jsonToString(value)
println(text)
releaseJsonText(text)
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
