# API Reference

This is the public reference for Seen's standard library and runtime. The
module index records every module currently present under `seen_std/src`; the
topic pages document the user-facing surfaces at a practical level.

## Standard Library (`seen_std/`)

The standard library is written in Seen and provides high-level abstractions.

| Module | Description |
|--------|-------------|
| [Core Types](core.md) | Option, Result, Unit, Ordering, type conversion |
| [Async](async.md) | Future, waker, task, and async runtime helpers |
| [Audio](audio.md) | Audio devices, formats, and backend wrappers |
| [Strings](string.md) | String operations, StringBuilder |
| [Collections](collections.md) | Array, Vec, HashMap, BTreeMap, ByteBuffer, sort/search helpers, priority queues |
| [Math](math.md) | Mathematical functions and constants |
| [I/O](io.md) | File I/O, stdio, buffered readers/writers |
| [Environment](env.md) | Environment helpers |
| [FFI](ffi.md) | C type metadata and C string interop |
| [Process](process.md) | Command execution, fork, environment variables |
| [Synchronization](sync.md) | Mutex, RwLock, Barrier, AtomicInt, Channel |
| [Threads](thread.md) | Thread, affinity, and worker-pool helpers |
| [Time](time.md) | Time and duration helpers |
| [Random](random.md) | LCG, PCG, Xorshift generators |
| [Bitfield](bitfield.md) | Bitfield8/16/32/64, network byte order |
| [Binary](binary.md) | Binary serialization, RLE compression, packets |
| [JSON](json.md) | JSON derive, parsing, generation |
| [Reflection](reflect.md) | @reflect RTTI, field introspection |
| [GPU](gpu.md) | Buffer, Uniform, Image types; Vulkan runtime |
| [Graphics](graphics.md) | GPU resource wrappers, renderer, shader helpers |
| [Input](input.md) | Gamepad state and events |
| [Memory](memory.md) | Allocation budgets plus mapped, pool, and stack regions |
| [Networking](net.md) | Poll and TCP wrappers |
| [Platform](platform.md) | Darwin, Linux, Windows, and Web bindings |
| [Scripting](scripting.md) | Lua integration |
| [Security](security.md) | TEE/enclave helpers |
| [SIMD](simd.md) | SIMD vector and math helpers |
| [UWW](uww.md) | Deterministic UWW and fixed-point helpers |
| [Hot Reload](hotreload.md) | Dynamic shared-module loading and typed entrypoint calls |
| [Full Module Index](stdlib-modules.md) | All `seen_std/src` modules grouped by family |

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
