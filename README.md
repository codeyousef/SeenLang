<p align="center">
  <img src="docs/images/seen-logo.png" alt="Seen Language" width="180">
</p>

<h1 align="center">Seen (س)</h1>

<p align="center">
  <strong>Native systems programming, written in the language your team thinks in.</strong>
</p>

<p align="center">
  Seen is a self-hosted systems language with LLVM code generation, a built-in
  language server, and first-class source keywords in English, Arabic, Spanish,
  Russian, Chinese, and Japanese.
</p>

<p align="center">
  <a href="#multilingual-by-design">Multilingual</a> &middot;
  <a href="#quick-start">Quick Start</a> &middot;
  <a href="#examples">Examples</a> &middot;
  <a href="#features">Features</a> &middot;
  <a href="#targets">Targets</a> &middot;
  <a href="#tooling">Tooling</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <a href="docs/cli-reference.md"><img src="https://img.shields.io/badge/release-0.9.4-2f855a.svg" alt="Seen 0.9.4"></a>
  <a href="docs/targets.md"><img src="https://img.shields.io/badge/backend-LLVM-5f43b2.svg" alt="LLVM backend"></a>
  <a href="docs/bootstrap.md"><img src="https://img.shields.io/badge/compiler-self--hosted-brightgreen.svg" alt="Self-hosted compiler"></a>
  <a href="docs/multilingual.md"><img src="https://img.shields.io/badge/languages-6-orange.svg" alt="6 keyword languages"></a>
</p>

---

## Multilingual by Design

Seen's lexer loads keyword and standard-library aliases from TOML language
packs, then hands language-neutral tokens to the parser and later compiler
stages. The same program can be written naturally in any shipped keyword set.

English:

```seen
fun main() {
    let names = ["Alice", "Bob", "Charlie"]
    for name in names {
        println("Hello, {name}!")
    }
}
```

Arabic:

```seen
دالة رئيسية() {
    اجعل أسماء = ["أحمد", "سارة", "خالد"]
    لكل اسم في أسماء {
        اطبع("مرحبا، {اسم}!")
    }
}
```

Chinese:

```seen
函数 主函数() {
    让 名字列表 = ["小明", "小红", "小华"]
    对于 名字 在 名字列表 {
        打印("你好，{名字}！")
    }
}
```

Compile with an explicit language flag, or put the language in `Seen.toml`:

```bash
seen compile hello_ar.seen hello --language ar
seen translate hello.seen --from en --to ja -o hello_ja.seen
```

```toml
[project]
name = "my_project"
language = "ar"
```

| Language | Code | `fun` | `let` | `return` | `println` | `main` |
|----------|------|-------|-------|----------|-----------|--------|
| English | `en` | `fun` | `let` | `return` | `println` | `main` |
| Arabic | `ar` | `دالة` | `اجعل` | `رجع` | `اطبع` | `رئيسية` |
| Spanish | `es` | `función` | `sea` | `retornar` | `imprimir` | `principal` |
| Russian | `ru` | `функция` | `пусть` | `вернуть` | `печать` | `главная` |
| Chinese | `zh` | `函数` | `让` | `返回` | `打印` | `主函数` |
| Japanese | `ja` | `関数` | `定数` | `戻る` | `表示` | `メイン` |

Full translation tables live in [docs/multilingual.md](docs/multilingual.md).

## Quick Start

```bash
echo 'fun main() { println("Hello, Seen!") }' > hello.seen
seen compile hello.seen hello
./hello
```

Common commands:

```bash
seen compile source.seen output
seen run source.seen
seen check source.seen
seen pkg fetch
seen lsp
```

The shipped compiler surface is documented in
[docs/cli-reference.md](docs/cli-reference.md). Source build and bootstrap notes
live in [docs/bootstrap.md](docs/bootstrap.md).

## Examples

Types and methods:

```seen
class Vec2 {
    var x: Float
    var y: Float

    static fun new(x: Float, y: Float) r: Vec2 {
        return Vec2 { x: x, y: y }
    }

    fun add(other: Vec2) r: Vec2 {
        return Vec2.new(this.x + other.x, this.y + other.y)
    }
}

fun main() {
    let a = Vec2.new(3.0, 4.0)
    let b = Vec2.new(1.0, 2.0)
    let c = a.add(b)
    println("Vector: {c.x}, {c.y}")
}
```

Pattern matching:

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

SIMD-oriented code:

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

## Features

- **Self-hosted compiler**: the compiler is written in Seen and verified through
  staged bootstrap.
- **Six keyword languages**: source can use English, Arabic, Spanish, Russian,
  Chinese, or Japanese keywords and selected standard-library aliases.
- **LLVM native code generation**: release LTO, target selection, PIC object
  output, package artifact linking, PGO controls, and sanitizer flags.
- **Region-oriented runtime APIs**: regions, arenas, allocation-budget tracking,
  fallible allocation paths, and diagnostics instead of raw host OOM failures.
- **Performance-focused standard library**: contiguous `Vec`, byte-backed
  `ByteArray`/`ByteBuffer`, numeric buffers, real map hashing, sort/search
  helpers, radix sort, and priority queues.
- **Tooling built in**: LSP, official VS Code extension, diagnostics,
  completions, snippets, package commands, and source translation.
- **Native systems surfaces**: C interop, SIMD helpers, GPU-facing APIs, package
  prebuilds, and platform packaging commands.

## Targets

The shipped `seen compile` target names are:

| Platform | Target |
|----------|--------|
| Linux x86-64 | `linux-x86_64` |
| Linux ARM64 | `linux-arm64` |
| Linux RISC-V 64 | `linux-riscv64` |
| Windows x86-64 | `windows-x86_64` |
| macOS Intel | `macos-x86_64` |
| macOS Apple Silicon | `macos-arm64` |
| iOS device | `ios-arm64` |
| iOS simulator | `ios-sim-arm64` |
| Android ARM64 | `android-arm64` |

`linux-riscv64` uses an RV64GC Linux GNU baseline with the LP64D ABI. The fast
verification tier builds RISC-V ELF binaries and runs them under QEMU user-mode;
an optional full-system QEMU script is available for guest-level validation.

More detail: [docs/targets.md](docs/targets.md).

## Tooling

The `vscode-seen/` directory contains the official VS Code extension with
syntax highlighting, LSP-backed diagnostics and completions, snippets, package
tasks, source translation, and multilingual project settings.

```bash
cd vscode-seen
npm install
npm run package
code --install-extension seen-*.vsix
```

Seen also works with any editor that can launch the built-in language server:

```bash
seen lsp
```

See [docs/tooling.md](docs/tooling.md) for editor setup and diagnostics.

## Benchmark Gates

Seen keeps small, capped performance gates for the compiler, stdlib, runtime,
release LTO, and packages. Current 0.9.4 baseline coverage includes:

| Suite | Maintained Gate Coverage |
|-------|--------------------------|
| `build` | quick-tier compiler rebuild timing, peak RSS, cache hits/misses |
| `stdlib` | collections, string/JSON, math, sort/search helpers |
| `runtime` | allocation-budget paths and SIMD reductions |
| `release-lto` | default merged LTO and explicit opt-out behavior |
| `packages` | Linux package artifact staging and reuse |

Run the maintained gates from the repository root:

```bash
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite stdlib
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite runtime
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite build --tier quick
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite release-lto
```

Baselines are stored under `target/seen-build/perf-baselines/`, and detailed
benchmark notes live in [benchmarks/README.md](benchmarks/README.md).

## Project Map

```text
SeenLang/
|-- compiler_seen/     self-hosted compiler, lexer/parser/typechecker/codegen/LSP
|-- bootstrap/         frozen bootstrap compiler and verification assets
|-- seen_std/          standard library written in Seen
|-- seen_runtime/      C runtime support
|-- languages/         six TOML keyword and stdlib alias packs
|-- vscode-seen/       official VS Code extension
|-- benchmarks/        production and comparison benchmark suites
|-- installer/         platform installer assets
|-- docs/              guides, references, and architecture notes
`-- scripts/           build, validation, packaging, and perf-gate tools
```

## Documentation

- [Getting Started](docs/getting-started.md)
- [Language Guide](docs/language-guide.md)
- [Multi-Language Support](docs/multilingual.md)
- [CLI Reference](docs/cli-reference.md)
- [Compiler Architecture](docs/compiler-architecture.md)
- [Known Limitations](docs/known-limitations.md)

## License

MIT License. See [LICENSE](LICENSE) for details.
