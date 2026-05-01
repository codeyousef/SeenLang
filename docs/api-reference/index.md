# API Reference

This is the complete reference for Seen's standard library and runtime.

## Standard Library (`seen_std/`)

The standard library is written in Seen and provides high-level abstractions.

| Module | Description |
|--------|-------------|
| [Core Types](core.md) | Option, Result, Unit, Ordering, type conversion |
| [Strings](string.md) | String operations, StringBuilder |
| [Collections](collections.md) | Array, Vec, HashMap, BTreeMap, HashSet, LinkedList, SmallVec |
| [Math](math.md) | Mathematical functions and constants |
| [I/O](io.md) | File I/O, stdio, buffered readers/writers |
| [Process](process.md) | Command execution, fork, environment variables |
| [Synchronization](sync.md) | Mutex, RwLock, Barrier, AtomicInt, Channel |
| [Random](random.md) | LCG, PCG, Xorshift generators |
| [Bitfield](bitfield.md) | Bitfield8/16/32/64, network byte order |
| [Binary](binary.md) | Binary serialization, RLE compression, packets |
| [JSON](json.md) | JSON derive, parsing, generation |
| [Reflection](reflect.md) | @reflect RTTI, field introspection |
| [GPU](gpu.md) | Buffer, Uniform, Image types; Vulkan runtime |
| [Hot Reload](hotreload.md) | Dynamic shared-module loading and typed entrypoint calls |

## Runtime Library (`seen_runtime/`)

The runtime is written in C and provides low-level primitives that every Seen program links against. It includes:

- ~170 functions covering strings, arrays, I/O, math, concurrency, SIMD, and memory management
- Automatically linked by the compiler (`-lm -lpthread`)
- Platform-specific implementations for Linux and macOS

## Prelude

The following are auto-imported into every Seen program:

- `println(s: String)` -- print string with newline
- `print(s: String)` -- print string without newline
- `assert(condition: Bool, message: String)` -- assertion
- `unreachable(message: String)` -- panic with unreachable message
- `Int_min()` / `Int_max()` -- integer bounds
- `Ok(value)` / `Err(error)` -- Result constructors
- `Some(value)` / `None()` -- Option constructors
- All primitive types: `Int`, `Float`, `Bool`, `String`, `Char`
- Collection types: `Array<T>`, `HashMap<K,V>`
