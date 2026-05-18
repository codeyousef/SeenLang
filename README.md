# Seen (س)

Seen is a self-hosted systems programming language for native applications,
tooling, games, and runtime-heavy systems code. The compiler is written in Seen,
uses LLVM for code generation, ships with a standard library and C runtime, and
supports source keywords in English, Arabic, Spanish, Russian, Chinese, and
Japanese.

```seen
fun main() {
    println("Hello, Seen!")
}
```

## Main Features

- Self-hosted compiler with staged bootstrap verification.
- LLVM native code generation with release LTO, cross-target compilation, PIC
  object output, package artifact linking, PGO controls, and sanitizer flags.
- Memory-safe runtime model with regions, arenas, allocation-budget tracking,
  fallible allocation APIs, and diagnostics instead of raw host OOM failures.
- Performance-oriented stdlib foundations, including contiguous `Vec`,
  byte-backed `ByteArray`/`ByteBuffer`, primitive numeric buffers, real map
  hashing, sort/search helpers, radix sort, and priority queues.
- Text, JSON, math, SIMD, and compiler hot paths routed through linear builders,
  byte scanners, runtime/libm helpers, bounded worker pools, and reusable caches.
- Built-in LSP plus the official VS Code extension for syntax highlighting,
  diagnostics, completions, snippets, package commands, and multilingual source.

## Compilation Targets

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

More detail: [docs/targets.md](docs/targets.md) and
[docs/cli-reference.md](docs/cli-reference.md).

## Benchmark Results

Seen keeps small, capped performance gates for the compiler, stdlib, runtime,
release LTO, and packages. Current 0.9.0 baseline coverage includes:

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
