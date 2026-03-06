# Seen Language Documentation

Seen is a self-hosted systems programming language with multi-language keywords, region-based memory management, and LLVM-powered compilation.

```seen
fun main() {
    let names = ["Alice", "Bob", "Charlie"]
    for name in names {
        println("Hello, {name}!")
    }
}
```

## Key Features

- **Self-hosted compiler** -- 62,000+ lines of Seen compiling itself
- **Multi-language keywords** -- write code in English, Arabic, Spanish, Russian, Chinese, or French
- **LLVM backend** -- ThinLTO, vectorization, aggressive inlining (1.0x--1.5x Rust performance)
- **Region-based memory** -- no garbage collector
- **Fast compilation** -- fork-parallel IR generation with content-addressed incremental caching
- **GPU compute** -- `@compute` shaders via Vulkan/GLSL pipeline
- **SIMD intrinsics** -- `f32x4`, `i32x4`, `f64x2`, etc.
- **Async/await** -- LLVM coroutine-based concurrency

## Documentation

### Getting Started

- [Getting Started](getting-started.md) -- installation, first program, project setup
- [Language Guide](language-guide.md) -- complete syntax and semantics reference
- [CLI Reference](cli-reference.md) -- every command and flag

### Core Concepts

- [Memory Model](memory-model.md) -- regions, ownership, borrowing, defer
- [Concurrency](concurrency.md) -- async/await, parallel_for, sync primitives
- [Metaprogramming](metaprogramming.md) -- comptime, decorators, derive macros, reflection

### Specialized Topics

- [SIMD and GPU](simd-and-gpu.md) -- vector types and GPU compute shaders
- [Multi-Language Support](multilingual.md) -- keyword translations and adding languages
- [FFI](ffi.md) -- C interop, extern functions, linking
- [Tooling](tooling.md) -- VS Code extension, LSP, formatting, debugging

### Project & Build

- [Project Configuration](project-config.md) -- Seen.toml format and project structure
- [Compiler Architecture](compiler-architecture.md) -- 5-stage pipeline internals
- [Bootstrap System](bootstrap.md) -- self-hosting verification and safe rebuilds
- [Known Limitations](known-limitations.md) -- current bugs and workarounds

### API Reference

- [API Reference Index](api-reference/index.md) -- standard library and runtime overview
- [Core Types](api-reference/core.md) -- Option, Result, Unit, Ordering
- [Strings](api-reference/string.md) -- String, StringBuilder
- [Collections](api-reference/collections.md) -- Array, Vec, HashMap, BTreeMap, etc.
- [Math](api-reference/math.md) -- math functions and constants
- [I/O](api-reference/io.md) -- File, stdin/stdout, reading and writing
- [Process](api-reference/process.md) -- Command execution, environment
- [Synchronization](api-reference/sync.md) -- Mutex, RwLock, Channel, AtomicInt
- [Random](api-reference/random.md) -- random number generators
- [Bitfield](api-reference/bitfield.md) -- bit manipulation types
- [Binary](api-reference/binary.md) -- serialization and compression
- [JSON](api-reference/json.md) -- JSON derive and parsing
- [Reflection](api-reference/reflect.md) -- runtime type information
- [GPU](api-reference/gpu.md) -- GPU types and runtime functions
