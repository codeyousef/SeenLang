<p align="center">
  <img src="docs/images/seen-logo.png" alt="Seen Language" width="180">
</p>

<h1 align="center">Seen (س)</h1>

<p align="center">
  <strong>A self-hosted systems programming language with multi-language keywords</strong>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> &middot;
  <a href="#examples">Examples</a> &middot;
  <a href="#language-features">Features</a> &middot;
  <a href="#benchmarks">Benchmarks</a> &middot;
  <a href="#ide-support">IDE Support</a> &middot;
  <a href="#contributing">Contributing</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <a href="#"><img src="https://img.shields.io/badge/Platform-Linux%20%7C%20macOS-lightgrey.svg" alt="Platform"></a>
  <a href="#"><img src="https://img.shields.io/badge/Compiler-Self--Hosted-brightgreen.svg" alt="Self-Hosted"></a>
  <a href="#"><img src="https://img.shields.io/badge/Languages-6-orange.svg" alt="6 Languages"></a>
</p>

---

Seen is a compiled systems programming language where the compiler is written entirely in Seen itself. It targets LLVM, ships with a built-in LSP, and lets you write code using keywords in English, Arabic, Spanish, Russian, Chinese, or Japanese.

```seen
fun main() {
    let names = ["Alice", "Bob", "Charlie"]
    for name in names {
        println("Hello, {name}!")
    }
}
```

The same program in Arabic:

```seen
دالة main() {
    ليكن names = ["Alice", "Bob", "Charlie"]
    لكل name في names {
        println("مرحبا، {name}!")
    }
}
```

## Why Seen?

**Performance** -- Seen compiles through LLVM with ThinLTO, vectorization, and aggressive inlining. Benchmarks track within 1.0x--1.5x of equivalent Rust programs across 17 workloads (matrix multiplication, sieves, binary trees, n-body simulation, etc.).

**Self-hosted** -- The compiler (62,000+ lines of Seen across 123 source files) compiles itself. Bootstrap verification confirms the fixed-point: stage 2 and stage 3 produce identical binaries.

**Fast compilation** -- Fork-parallel IR generation across 50+ modules with content-addressed incremental caching. Only changed modules recompile.

**Multi-language keywords** -- Keywords are defined in TOML files under `languages/`. Adding a new language is adding a directory of TOML files -- no compiler changes required.

**Region-based memory** -- No garbage collector. Memory is managed through regions and arenas with compile-time lifetime tracking.

## Quick Start

### Prerequisites

- **LLVM 18+** (clang, opt, lld)
- **GCC** (for runtime compilation)
- **Git**

### Build from Source

```bash
git clone https://github.com/codeyousef/SeenLang.git
cd SeenLang
./scripts/safe_rebuild.sh
```

The production compiler lands at `compiler_seen/target/seen`.

### Install

```bash
sudo cp compiler_seen/target/seen /usr/local/bin/seen
```

Or add to your shell profile:

```bash
export PATH="$PATH:/path/to/SeenLang/compiler_seen/target"
```

### Hello World

```bash
echo 'fun main() { println("Hello, Seen!") }' > hello.seen
seen build hello.seen -o hello
./hello
```

## Usage

```bash
seen build source.seen -o output   # Compile to native binary
seen build source.seen --fast      # Fast build (skip Polly, O1)
seen run source.seen               # JIT execution
seen check source.seen             # Type check only
seen fmt source.seen               # Format code
seen lsp                           # Start language server
```

### Compiler Flags

| Flag | Description |
|------|-------------|
| `--fast` | Skip heavy optimizations, use O1 |
| `--release` | Full optimization with LTO |
| `--emit-llvm` | Dump generated LLVM IR |
| `--backend c` | Use C backend instead of LLVM |
| `--debug` | Enable debug symbols and tracing |
| `--trace-llvm` | Trace LLVM IR generation |
| `--dump-struct-layouts` | Print struct field layouts |
| `--null-safety` | Enable null safety checks |
| `--warn-uninit` | Warn on uninitialized variables |
| `--stack-check` | Enable stack overflow checks |

## Examples

### Variables and Control Flow

```seen
fun main() {
    let name = "Seen"
    var count = 0

    while count < 5 {
        count = count + 1
        if count == 3 {
            println("Three!")
        }
    }

    println("{name}: counted to {count}")
}
```

### Classes and Methods

```seen
class Vec2 {
    var x: Float
    var y: Float

    static fun new(x: Float, y: Float) r: Vec2 {
        return Vec2 { x: x, y: y }
    }

    fun length() r: Float {
        return sqrt(this.x * this.x + this.y * this.y)
    }

    fun add(other: Vec2) r: Vec2 {
        return Vec2.new(this.x + other.x, this.y + other.y)
    }
}

fun main() {
    let a = Vec2.new(3.0, 4.0)
    let b = Vec2.new(1.0, 2.0)
    let c = a.add(b)
    println("Length: {c.length()}")
}
```

### Enums and Pattern Matching

```seen
enum Shape {
    Circle(radius: Float)
    Rectangle(width: Float, height: Float)
}

fun area(shape: Shape) r: Float {
    return when shape {
        is Circle(r) => 3.14159 * r * r
        is Rectangle(w, h) => w * h
    }
}
```

### Traits

```seen
trait Printable {
    fun display() r: String
}

impl Printable for Vec2 {
    fun display() r: String {
        return "({this.x}, {this.y})"
    }
}
```

### Generics

```seen
fun max<T>(a: T, b: T) r: T {
    if a > b { return a }
    return b
}

class Stack<T> {
    var items: Array<T>

    fun push(item: T) {
        this.items.push(item)
    }

    fun pop() r: T {
        return this.items.pop()
    }
}
```

### Async/Await

```seen
@async
fun fetchData(url: String) r: String {
    let response = await httpGet(url)
    return response.body
}
```

### Closures

```seen
fun apply(arr: Array<Int>, f: Fun) r: Array<Int> {
    var result = Array<Int>()
    for item in arr {
        result.push(f(item))
    }
    return result
}

fun main() {
    let nums = [1, 2, 3, 4, 5]
    let doubled = apply(nums, |x| x * 2)
}
```

### SIMD

```seen
fun dot_product(a: Array<Float>, b: Array<Float>, n: Int) r: Float {
    var sum = f32x4(0.0, 0.0, 0.0, 0.0)
    var i = 0
    while i + 4 <= n {
        let va = simd_load_f32x4(a, i)
        let vb = simd_load_f32x4(b, i)
        sum = sum + va * vb
        i = i + 4
    }
    return reduce_add(sum)
}
```

### GPU Compute (Vulkan)

```seen
@compute(workgroup_size = 64)
fun vector_add(a: Buffer<Float>, b: Buffer<Float>, out: Buffer<Float>) {
    let idx = global_invocation_id.x
    out[idx] = a[idx] + b[idx]
}
```

### Parallel For

```seen
fun main() {
    var results = Array<Int>.withLength(1000)
    parallel_for i in 0..1000 {
        results[i] = i * i
    }
}
```

### Compile-Time Evaluation

```seen
comptime fun factorial(n: Int) r: Int {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}

let TABLE_SIZE = comptime { factorial(10) }
```

### Defer and Error Handling

```seen
fun readFile(path: String) r: String {
    let file = File.open(path)
    defer { file.close() }

    try {
        return file.readAll()
    } catch e {
        println("Error: {e}")
        return ""
    }
}
```

## Language Features

### Type System
- Immutable by default (`let`), opt-in mutability (`var`)
- Nullable types (`T?`) with safe access (`?.`) and null coalescing (`??`)
- Generics with constraints (`<T: Ord>`)
- Type aliases and distinct types
- `Result<T, E>` and `Option<T>` types

### Data Structures
- Classes with methods, inheritance, and traits
- Enums (simple and data-carrying)
- Structs (value types)
- `Array<T>`, `Vec<T>`, `HashMap<K, V>`, `BTreeMap<K, V>`, `LinkedList<T>`, `SmallVec<T, N>`

### Memory Management
- Region-based memory (no GC)
- `move`, `borrow`, `ref` semantics
- `defer` for cleanup
- `arena` allocators
- `@packed`, `@cache_line` layout control

### Concurrency
- `async`/`await` with LLVM coroutines
- `parallel_for` with fork-based parallelism
- `Mutex`, `RwLock`, `Barrier`, `Channel`, `AtomicInt`
- `@send`/`@sync` markers for thread safety

### Metaprogramming
- `comptime` evaluation
- Decorators: `@derive(Clone, Hash, Eq, Debug, Serialize, Deserialize, Json)`
- `@reflect` for runtime type information
- `@intrinsic` for LLVM intrinsic mapping

### GPU
- `@compute`, `@vertex`, `@fragment` shader annotations
- `Buffer<T>`, `Uniform<T>`, `Image<T>` types
- GLSL codegen with Vulkan runtime
- `--emit-glsl` to inspect generated shaders

### SIMD
- Vector types: `i8x16`, `i16x8`, `i32x4`, `i64x2`, `f32x4`, `f64x2`
- Arithmetic, comparison, shuffle, swizzle
- Horizontal reductions (`reduce_add`, `reduce_min`, `reduce_max`)
- Aligned load/store, gather/scatter

### Interop
- `extern fun` for C FFI
- `@cImport` for C header inclusion
- `@repr(C)` for C-compatible struct layout

### Operators
- Word operators: `and`, `or`, `not` (alongside `&&`, `||`, `!`)
- String interpolation: `"Hello, {name}!"`
- Range: `0..n`, `0..=n`
- Pipe-style chaining

## Benchmarks

17 production benchmarks in `benchmarks/production/`:

| Benchmark | Description |
|-----------|-------------|
| `01_matrix_mult` | Dense matrix multiplication |
| `02_sieve` | Sieve of Eratosthenes |
| `03_binary_trees` | GC-stress binary tree allocation |
| `04_fasta` | FASTA sequence generation |
| `05_nbody` | N-body planetary simulation |
| `06_revcomp` | Reverse-complement DNA |
| `07_mandelbrot` | Mandelbrot set rendering |
| `08_lru_cache` | LRU cache with hash map |
| `09_json_serialize` | JSON serialization |
| `11_spectral_norm` | Spectral norm computation |
| `12_fannkuch` | Fannkuch-redux permutations |
| `13_great_circle` | Great-circle distance |
| `14_hyperbolic_pde` | Hyperbolic PDE solver |
| `15_dft_spectrum` | Discrete Fourier transform |
| `16_euler_totient` | Euler's totient function |
| `17_fibonacci` | Recursive Fibonacci |

Run benchmarks:

```bash
./scripts/run_production_benchmarks.sh
```

Comparison benchmarks against C, C++, Rust, and Zig are in `benchmarks/comparison/`.

## Multi-Language Support

Seen's keywords are defined externally in TOML files. Six languages ship with the compiler:

| Language | Directory | Example keyword for `fun` |
|----------|-----------|---------------------------|
| English | `languages/en/` | `fun` |
| Arabic | `languages/ar/` | `دالة` |
| Spanish | `languages/es/` | `fun` |
| Russian | `languages/ru/` | `функция` |
| Chinese | `languages/zh/` | `函数` |
| Japanese | `languages/ja/` | `関数` |

Each language has 17 TOML files covering keywords, operators, and standard library names.

### Adding a New Language

1. Create `languages/xx/` (where `xx` is the language code)
2. Copy the English TOML files as templates
3. Translate keyword values
4. The compiler auto-detects available languages

No compiler rebuild required.

## IDE Support

### Visual Studio Code

The `vscode-seen/` directory contains a full-featured extension:

- Syntax highlighting with TextMate grammar
- IntelliSense via built-in LSP
- Real-time error diagnostics
- Code formatting, debugging, REPL
- Snippets for common patterns
- Multi-language keyword support

```bash
cd vscode-seen
npm install
npm run package
code --install-extension seen-*.vsix
```

### Any Editor (LSP)

Seen includes a built-in language server:

```bash
seen lsp
```

**Neovim:**
```lua
require'lspconfig'.seen.setup{
  cmd = {"seen", "lsp"},
  filetypes = {"seen"},
  root_dir = require'lspconfig.util'.root_pattern("Seen.toml", ".git"),
}
```

**Emacs:**
```elisp
(lsp-register-client
 (make-lsp-client :new-connection (lsp-stdio-connection '("seen" "lsp"))
                  :major-modes '(seen-mode)
                  :server-id 'seen-lsp))
```

## Project Structure

```
SeenLang/
├── compiler_seen/            # Self-hosted compiler (62K+ lines of Seen)
│   └── src/
│       ├── main.seen         # Entry point
│       ├── lexer/            # Tokenizer with multi-language support
│       ├── parser/           # Recursive descent parser
│       ├── typechecker/      # Type inference and checking
│       ├── codegen/          # LLVM IR generation (13 modules)
│       ├── ir/               # IR builder and SSA construction
│       ├── bootstrap/        # Frontend orchestration
│       └── lsp/              # Language server implementation
├── bootstrap/                # Frozen bootstrap compiler
│   └── stage1_frozen         # Verified binary (SHA-256 checked)
├── seen_std/                 # Standard library (Seen)
├── seen_runtime/             # C runtime (memory, I/O, collections)
├── languages/                # Keyword definitions (6 languages, 102 TOML files)
├── vscode-seen/              # VS Code extension
├── tests/                    # Test suites
│   └── e2e_multilang/        # 66 end-to-end tests across 6 languages
├── benchmarks/               # 17 production benchmarks + comparison suite
├── scripts/                  # Build, test, and IR validation tools
├── installer/                # Platform installers (Linux, macOS, Windows)
└── docs/                     # Design documents and specifications
```

## Compiler Architecture

The compiler follows a 5-stage pipeline:

```
Source (.seen)
  → Lexer (tokenize with language-specific keywords)
  → Parser (recursive descent → AST)
  → Type Checker (inference, validation, smart casts)
  → IR Generator (AST → LLVM IR, three-pass: signatures → types → bodies)
  → LLVM Backend (opt -O3 → ThinLTO → lld link)
  → Native Binary
```

Key architectural decisions:
- **Fork-parallel codegen**: Each module's IR is generated in a forked child process with copy-on-write memory
- **Content-addressed IR cache**: Cache key = `hash(declarations_digest + module_source)`, so editing one function only recompiles that module
- **Three-pass IR generation**: First pass collects all signatures, second resolves types, third emits function bodies -- enables forward references without a separate declaration phase
- **IR validation**: `scripts/seen_ir_verify.sh` runs `llvm-as` structural checks and `seen_ir_lint` semantic checks on every `.ll` file before optimization

## Development

### Bootstrap-Verified Builds

The compiler compiles itself. After any change to `compiler_seen/src/`, verify bootstrap:

```bash
./scripts/safe_rebuild.sh
```

This builds stage 2 from the frozen bootstrap, then stage 3 from stage 2. If stage 2 == stage 3, the fixed-point is confirmed.

### Running Tests

```bash
# End-to-end tests (66 tests, 6 languages)
bash tests/e2e_multilang/run_all_e2e.sh

# IR validation on generated modules
./scripts/seen_ir_verify.sh /tmp/seen_module_*.ll
```

### Debugging the Compiler

```bash
# Type checker tracing
SEEN_DEBUG_TYPES=1 seen build program.seen

# LLVM IR generation tracing
SEEN_TRACE_LLVM=all seen build program.seen

# Struct layout debugging
SEEN_TRACE_LLVM=gep seen build program.seen
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes
4. Run tests: `bash tests/e2e_multilang/run_all_e2e.sh`
5. Verify bootstrap: `./scripts/safe_rebuild.sh`
6. Submit a pull request

## License

MIT License. See [LICENSE](LICENSE) for details.

---

<p align="center">
  <a href="#quick-start">Get Started</a> &middot;
  <a href="https://github.com/codeyousef/SeenLang/issues">Report a Bug</a> &middot;
  <a href="https://github.com/codeyousef/SeenLang">Star on GitHub</a>
</p>
