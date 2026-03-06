# Strings

## String

Strings in Seen are UTF-8 encoded, represented as `{len: Int, data: ptr}`.

### String Interpolation

```seen
let name = "World"
println("Hello, {name}!")
println("2 + 2 = {2 + 2}")
```

### Operators

```seen
let greeting = "Hello, " + "World!"     // concatenation
let equal = "abc" == "abc"              // equality
let notEqual = "abc" != "xyz"           // inequality
```

### Length

```seen
let s = "hello"
let len = s.length()           // byte length: 5
```

### Character Access

```seen
let ch = seen_char_at(s, 0)   // Unicode code point at index
let byte = seen_byte_at(s, 0) // raw byte at index
```

### String Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `split` | `(text: String, delimiter: String) r: Array<String>` | Split by delimiter |
| `trim` | `(text: String) r: String` | Remove leading/trailing whitespace |
| `startsWith` | `(text: String, prefix: String) r: Bool` | Check prefix |
| `endsWith` | `(text: String, suffix: String) r: Bool` | Check suffix |
| `contains` | `(text: String, needle: String) r: Bool` | Check substring |
| `indexOf` | `(text: String, needle: String, start: Int) r: Int` | Find first occurrence (-1 if not found) |

### Runtime String Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `seen_str_concat_ss` | `(a: String, b: String) r: String` | Concatenate |
| `seen_str_eq_ss` | `(a: String, b: String) r: Bool` | Equality |
| `seen_substring` | `(s: String, start: Int, end: Int) r: String` | Substring extraction |
| `seen_int_to_string` | `(n: Int) r: String` | Int to string |
| `seen_float_to_string` | `(f: Float) r: String` | Float to string |
| `seen_char_to_str` | `(c: Int) r: String` | Char code point to string |

### Stdlib String Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `String_toUpperCase` | `(s: String) r: String` | Uppercase conversion |
| `String_toLowerCase` | `(s: String) r: String` | Lowercase conversion |
| `String_toInt` | `(s: String) r: Int` | Parse as integer |
| `String_toFloat` | `(s: String) r: Float` | Parse as float |
| `String_reverse` | `(s: String) r: String` | Reverse string |
| `String_isEmpty` | `(s: String) r: Int` | Check if empty |
| `String_count` | `(s: String, needle: String) r: Int` | Count occurrences |
| `String_replace` | `(s: String, old: String, new: String) r: String` | Replace all occurrences |
| `String_fromCharCode` | `(code: Int) r: String` | Create from Unicode code point |

### Extended String Functions (seen_std/src/str/)

| Function | Signature | Description |
|----------|-----------|-------------|
| `trimStart` | `(s: String) r: String` | Trim leading whitespace |
| `trimEnd` | `(s: String) r: String` | Trim trailing whitespace |
| `lastIndexOf` | `(s: String, needle: String) r: Int` | Last occurrence |
| `lines` | `(s: String) r: Array<String>` | Split into lines |
| `splitWhitespace` | `(s: String) r: Array<String>` | Split on whitespace |
| `words` | `(s: String) r: Array<String>` | Split into words |
| `replaceFirst` | `(s: String, old: String, new: String) r: String` | Replace first occurrence |
| `removePrefix` | `(s: String, prefix: String) r: String` | Remove prefix if present |
| `removeSuffix` | `(s: String, suffix: String) r: String` | Remove suffix if present |
| `ensurePrefix` | `(s: String, prefix: String) r: String` | Add prefix if missing |
| `ensureSuffix` | `(s: String, suffix: String) r: String` | Add suffix if missing |
| `padStart` | `(s: String, len: Int, ch: Char) r: String` | Pad start |
| `padEnd` | `(s: String, len: Int, ch: Char) r: String` | Pad end |
| `repeat` | `(s: String, n: Int) r: String` | Repeat N times |
| `join` | `(arr: Array<String>, sep: String) r: String` | Join with separator |

## StringBuilder

Efficient string building with amortized allocation:

```seen
import str.string
```

### Constructor

```seen
let sb = StringBuilder.new()
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `append(text: String)` | `Void` | Append string |
| `appendChar(ch: Char)` | `Void` | Append character |
| `appendLine(text: String)` | `Void` | Append string + newline |
| `appendAll(values: Array<String>)` | `Void` | Append all strings |
| `clear()` | `Void` | Clear contents |
| `isEmpty()` | `Bool` | Check if empty |
| `length()` | `Int` | Get total length |
| `toString()` | `String` | Build final string |
| `buildAndClear()` | `String` | Build string and clear builder |

### Example

```seen
let sb = StringBuilder.new()
sb.append("Hello")
sb.append(", ")
sb.append("World!")
let result = sb.toString()  // "Hello, World!"
```

## Character Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `Char_isDigit` | `(c: Int) r: Int` | Is digit [0-9] |
| `Char_isAlpha` | `(c: Int) r: Int` | Is alphabetic [A-Za-z] |
| `Char_isAlphanumeric` | `(c: Int) r: Int` | Is alphanumeric |
| `Char_isUpperCase` | `(c: Int) r: Int` | Is uppercase |
| `Char_isLowerCase` | `(c: Int) r: Int` | Is lowercase |
| `Char_isWhitespace` | `(c: Int) r: Int` | Is whitespace |
| `Char_toUpperCase` | `(c: Int) r: Int` | To uppercase |
| `Char_toLowerCase` | `(c: Int) r: Int` | To lowercase |
| `Char_toInt` | `(c: Int) r: Int` | Character to integer (identity) |
