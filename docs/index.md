# Seen Language Documentation

Seen is a self-hosted systems programming language with multi-language keywords,
package support, region-oriented runtime APIs, and LLVM-powered compilation.

```seen
fun main() {
    let names = ["Alice", "Bob", "Charlie"]
    for name in names {
        println("Hello, {name}!")
    }
}
```

## Key Features

- **Self-hosted compiler** -- the compiler is written in Seen and verified through staged bootstrap
- **Multi-language keywords** -- write code in English, Arabic, Spanish, Russian, Chinese, or Japanese
- **LLVM backend** -- native code generation, optimization, cross-target support, and package artifact linking
- **Packages** -- source registry packages plus local prebuilt artifacts with interface indexes
- **Tooling** -- built-in LSP, VS Code extension, C import generation, platform packaging helpers
- **GPU/SIMD APIs** -- stdlib and runtime surfaces for graphics, compute, and vectorized code paths

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
- [Compilation Targets](targets.md) -- native and cross-target names, triples, RISC-V/QEMU verification

### Project & Build

- [Project Configuration](project-config.md) -- Seen.toml format and project structure
- [Packaging](packaging.md) -- static registry layout, publishing, and hosting
- [Compiler Architecture](compiler-architecture.md) -- frontend, codegen, backend, and bootstrap internals
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
- [Full Module Index](api-reference/stdlib-modules.md) -- every `seen_std/src` module family
