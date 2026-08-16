# Foreign Function Interface (FFI)

Seen interoperates with C through `extern` function declarations and the runtime C library.

## Calling C from Seen

### extern fun

Declare C functions with `extern fun`:

```seen
extern fun malloc(size: UInt64) r: *Void
extern fun free(value: *Void)
extern fun strlen(value: *const UInt8) r: UInt64
```

Use pointer types for C pointer parameters. A Seen `String` is the two-field
`SeenString` value described below; it is not implicitly a NUL-terminated
`char *`. Convert or wrap C strings explicitly and verify every declaration
against the C header for the selected target.

Compiler and standard-library internals still contain legacy `__`-prefixed
empty-body declarations. They are a bootstrap compatibility path, not the
public FFI grammar. Application code should use explicit `extern fun`:

```seen
fun __OpenFile(path: String, mode: String) r: Int {}
fun __ReadFile(fd: Int) r: String {}
fun __CloseFile(fd: Int) r: Int {}
```

### Using C functions

```seen
extern fun seen_memory_used_bytes() r: Int

fun main() {
    println("Seen-managed bytes: {seen_memory_used_bytes()}")
}
```

### Linking C libraries

Pass linker flags via the compiler:

```bash
seen compile app.seen app    # automatically links -lm -lpthread
```

Native Linux links the math and pthread runtime dependencies by default. Other
targets use their target-specific runtime libraries; do not add POSIX link
assumptions to Windows, Android, or Apple builds.

Additional libraries (e.g., `-lvulkan`) are added when GPU features are used.

### Project-local system libraries

For native shims that live inside your project, declare the library in `Seen.toml` and add a local search path:

```toml
[native.dependencies]
seen_platform = { path = "native/lib" }
```

`path` is resolved relative to the nearest `Seen.toml`. Seen adds `-L<resolved-path>` during linking. On native Linux/macOS builds it also records that directory as a runtime search path, so `seen compile` outputs run without extra `LIBRARY_PATH` or `LD_LIBRARY_PATH` wrappers.

Legacy `{ system = true }` entries under `[dependencies]` remain accepted, but
new manifests should use `[native.dependencies]`.

## Importing C declarations

Generate Seen bindings from a C header file:

```bash
seen import-c <header.h>
```

This parses the C header and outputs `extern fun` declarations.

## Exposing Seen to C

Seen declarations exposed to C need an ABI reviewed against emitted LLVM IR.
`pub` controls Seen visibility; it is not by itself a native export. Use
`@export` to preserve an unmangled symbol, `@repr(C)` for supported
layout-sensitive data, and inspect the object signature before publishing a
header.

```seen
@export
fun add(a: Int, b: Int) r: Int {
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
    char* data;
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

The standard library currently provides bootstrap compatibility wrappers in
`seen_std/src/ffi/`:

```seen
import ffi.cinterop.{CString, toCString, fromCString}

let wrapped = toCString("hello")
let text = fromCString(wrapped)
```

`CString` currently stores a Seen `String`; `toCString` does not allocate a
NUL-terminated `char *`. At a real C boundary, use the reviewed runtime
conversion helpers (`seen_str_to_cstr` returns an allocated copy) or a native
shim with an explicit ownership contract.

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
