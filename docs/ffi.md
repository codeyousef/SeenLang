# Foreign Function Interface (FFI)

Seen interoperates with C through `extern` function declarations and the runtime C library.

## Calling C from Seen

### extern fun

Declare C functions with `extern fun`:

```seen
extern fun printf(format: String) r: Int
extern fun malloc(size: Int) r: Int
extern fun free(ptr: Int)
extern fun strlen(s: String) r: Int
```

Functions starting with `__` and having an empty body are treated as external declarations:

```seen
fun __OpenFile(path: String, mode: String) r: Int {}
fun __ReadFile(fd: Int) r: String {}
fun __CloseFile(fd: Int) r: Int {}
```

### Using C functions

```seen
extern fun time(t: Int) r: Int

fun main() {
    let now = time(0)
    println("Unix timestamp: {now}")
}
```

### Linking C libraries

Pass linker flags via the compiler:

```bash
seen build app.seen -o app    # automatically links -lm -lpthread
```

The compiler always links:
- `-lm` (math library)
- `-lpthread` (POSIX threads)

Additional libraries (e.g., `-lvulkan`) are added when GPU features are used.

### Project-local system libraries

For native shims that live inside your project, declare the library in `Seen.toml` and add a local search path:

```toml
[dependencies]
seen_platform = { system = true, path = "native/lib" }
```

`path` is resolved relative to the nearest `Seen.toml`. Seen adds `-L<resolved-path>` during linking, and on native Linux/macOS builds it also records that directory as a runtime search path so raw `seen build` outputs can run without wrapper-set `LIBRARY_PATH` or `LD_LIBRARY_PATH`.

## @cImport

Generate Seen bindings from a C header file:

```bash
seen import-c <header.h>
```

This parses the C header and outputs `extern fun` declarations.

## Exposing Seen to C

Seen functions can be called from C when compiled as a library. The function name is preserved in the generated object file.

```seen
pub fun add(a: Int, b: Int) r: Int {
    return a + b
}
```

From C:

```c
#include <stdint.h>
extern int64_t add(int64_t a, int64_t b);

int main() {
    int64_t result = add(3, 4);
    printf("%lld\n", result);
}
```

## C-Compatible Layout

Use `@repr(C)` for C-compatible struct layout:

```seen
@repr(C)
class NetworkPacket {
    var version: Int
    var flags: Int
    var payload_size: Int
}
```

## Type Mapping

| Seen Type | C Type | LLVM IR |
|-----------|--------|---------|
| `Int` | `int64_t` | `i64` |
| `Float` | `double` | `double` |
| `Bool` | `bool` / `int64_t` | `i1` / `i64` |
| `String` | `SeenString` (struct) | `{i64, ptr}` |
| `Array<T>` | `SeenArray*` | `ptr` |

### SeenString layout

```c
typedef struct {
    int64_t len;
    const char* data;
} SeenString;
```

### SeenArray layout

```c
typedef struct {
    int64_t len;
    int64_t cap;
    int64_t element_size;
    void* data;
} SeenArray;
```

## Runtime C Library

The Seen runtime (`seen_runtime/seen_runtime.c`) provides ~170 C functions that are linked with every Seen program. These handle:

- Memory allocation
- String operations (`seen_str_concat_ss`, `seen_str_eq_ss`, etc.)
- Array operations (`seen_arr_push_*`, `seen_arr_get_*`, etc.)
- File I/O (`__OpenFile`, `__ReadFile`, etc.)
- Process management (`__seen_fork`, `__seen_waitpid`, etc.)
- Synchronization primitives (`seen_rwlock_*`, `seen_barrier_*`, etc.)
- SIMD operations (`seen_simd_f4_*`, `seen_simd_f8_*`, etc.)
- Arena/pool/region allocators

## C Interop Utilities

The standard library provides helpers in `seen_std/src/ffi/`:

```seen
import ffi

let cstr = toCString("hello")    // Seen String → CString
let str = fromCString(cstr)       // CString → Seen String
```

Type mapping helpers:

```seen
let seen_type = cTypeToSeen("int64_t")     // "Int"
let c_type = seenTypeToC("Int")            // "int64_t"
let size = getCTypeSize("int64_t")          // 8
let align = getCTypeAlignment("int64_t")    // 8
```

## Related

- [CLI Reference](cli-reference.md) -- build flags and linking
- [API Reference: Core](api-reference/core.md) -- built-in types
- [Compiler Architecture](compiler-architecture.md) -- how extern is compiled
