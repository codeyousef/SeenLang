# CLI Reference

## Subcommands

### `seen build`

Compile a Seen source file to a native binary.

```bash
seen build <source.seen> [-o output] [flags]
```

| Option | Description |
|--------|-------------|
| `-o, --output <file>` | Output file path |
| `-t, --target <target>` | Compilation target (see [Targets](#compilation-targets)) |
| `--backend <backend>` | `llvm` (default) or `c` |
| `-l, --language <lang>` | Source language: `en`, `ar`, `es`, `ru`, `zh`, `ja` |

### `seen run`

Compile and execute immediately via JIT:

```bash
seen run <source.seen> [--language <lang>]
```

Uses `lli --jit-kind=orc` by default. Pass `--aot` for ahead-of-time compilation.

### `seen check`

Type-check without generating code:

```bash
seen check <source.seen> [--language <lang>]
```

### `seen fmt` / `seen format`

Format source code (4-space indent, trim trailing whitespace):

```bash
seen fmt <source.seen>
```

### `seen init`

Create a new project scaffold:

```bash
seen init <project-name>
```

Creates `project-name/Seen.toml` and `project-name/src/main.seen`.

### `seen test`

Run the project's test suite:

```bash
seen test
```

### `seen clean`

Remove build artifacts (`.seen_cache/`, `/tmp/seen_module_*`, `/tmp/seen_jit_*`):

```bash
seen clean
```

### `seen lsp`

Start the built-in language server:

```bash
seen lsp
```

### `seen import-c`

Generate Seen bindings from a C header file:

```bash
seen import-c <header.h>
```

### `seen --version`

```bash
seen --version
# Seen 1.0.0-alpha (100% self-hosted)
```

## Optimization Flags

| Flag | Description |
|------|-------------|
| `-O0` | No optimization |
| `-O1` | Light optimization |
| `-O2` | Medium optimization (default) |
| `-O3` | Aggressive optimization |
| `--release` | Alias for `-O3` with full LTO |
| `--fast` | Skip Polly, use minimal `default<O1>` passes (fast compilation) |
| `-g, --debug` | Include debug information |

## Backend and Target Flags

| Flag | Description |
|------|-------------|
| `--backend llvm` | LLVM backend (default, best performance) |
| `--backend c` | C99 fallback backend |
| `--target-cpu=<cpu>` | Target CPU: `native`, `x86-64-v3`, `x86-64-v4` |
| `--target=<platform>` | Cross-compile target (see below) |

### Compilation Targets

| Target | Description |
|--------|-------------|
| `native` | Host platform (default) |
| `wasm` | WebAssembly module |
| `c` | C source code output |
| `llvm-ir` | LLVM IR output |
| `ios-arm64` | iOS ARM64 |
| `ios-sim-arm64` | iOS Simulator ARM64 |
| `macos-x86_64` | macOS x86_64 |
| `macos-arm64` | macOS ARM64 |
| `windows-x86_64` | Windows x86_64 |
| `linux-arm64` | Linux ARM64 |
| `riscv64` | RISC-V 64-bit |

## SIMD Flags

| Flag | Description |
|------|-------------|
| `--simd=auto` | Auto-detect (default) |
| `--simd=none` | Disable SIMD |
| `--simd=sse4.2` | Force SSE 4.2 |
| `--simd=avx2` | Force AVX2 |
| `--simd=avx512` | Force AVX-512 |
| `--simd-report` | Show vectorization report |
| `--simd-report=full` | Detailed per-loop report |

## Safety Flags

| Flag | Description |
|------|-------------|
| `--null-safety` | Enable null pointer safety checks |
| `--warn-uninit` | Warn on uninitialized variable access |
| `--stack-check` | Enable stack overflow checks |
| `--bounds-check` | Enable array bounds checking |
| `--panic-on-overflow` | Panic on integer overflow |
| `--warn-unused-result` | Warn on unused function results |

## Sanitizer Flags

```bash
seen build source.seen --sanitize=address    # AddressSanitizer
seen build source.seen --sanitize=undefined  # UBSan
seen build source.seen --sanitize=thread     # ThreadSanitizer
seen build source.seen --sanitize=memory     # MemorySanitizer
```

## Debug and Emission Flags

| Flag | Description |
|------|-------------|
| `--emit-llvm` | Save LLVM IR alongside output |
| `--emit-glsl` | Emit GLSL shader code (GPU) |
| `--emit-compile-db` | Generate `compile_commands.json` |
| `--trace-llvm` | Trace LLVM IR generation |
| `--dump-struct-layouts` | Print struct field layouts |
| `--runtime-debug` | Enable runtime debug output |

## Profile-Guided Optimization (PGO)

```bash
# Step 1: Generate profiling instrumentation
seen build app.seen -o app --pgo-generate
./app  # Run with representative input

# Step 2: Merge profiling data
llvm-profdata merge -o default.profdata default_*.profraw

# Step 3: Recompile with profile data
seen build app.seen -o app --pgo-use=default.profdata
```

## Machine Learning Integration

| Flag | Description |
|------|-------------|
| `--ml-log=<path>` | Collect ML training data from optimization remarks |
| `--ml-decision-log=<path>` | Write optimization decision log (JSONL) |

## Compilation Profiles

```bash
seen build app.seen --profile default        # Allow all types
seen build app.seen --profile deterministic  # Reject HashMap without @nondeterministic
seen build app.seen --deterministic          # Scalar SIMD, no HashMap
```

## Feature Flags

```bash
seen build app.seen --feature=my_feature --feature=experimental
```

Use with `@cfg` in source:

```seen
@cfg("my_feature")
fun experimentalFunction() { ... }
```

## Cache Control

| Flag | Description |
|------|-------------|
| `--no-cache` | Disable incremental compilation caching |
| `--no-fork` | Disable fork-parallel IR generation |

Cache locations:
- `.seen_cache/` -- source-level incremental cache
- `/tmp/seen_ir_cache` -- IR content-addressed cache
- `/tmp/seen_thinlto_cache` -- ThinLTO linker cache

## macOS-Specific Commands

### `seen bundle`

Create a macOS app bundle:

```bash
seen bundle <executable> <AppName> [--icon=<icon.icns>] [--version=<1.0>]
```

### `seen sign`

Code sign a binary or app bundle:

```bash
seen sign <path> [--identity=<identity>]
```

### `seen notarize`

Submit for Apple notarization:

```bash
seen notarize <path.zip|path.app> --apple-id=<email> --team-id=<id> --password=<pwd>
```

### `seen lipo`

Create universal binary:

```bash
seen lipo <x86_64_binary> <arm64_binary> [--output=<universal>]
seen lipo --from-source <source.seen> [--output=<universal>]
```

### `seen ipa`

Create iOS IPA package:

```bash
seen ipa <executable> <AppName> [--bundle-id=...] [--version=...] [--provisioning-profile=...]
```

## Environment Variables

| Variable | Values | Description |
|----------|--------|-------------|
| `SEEN_DEBUG_TYPES` | `1` | Type checker debug output |
| `SEEN_TRACE_LLVM` | `all`, `inst`, `values`, `types`, `ir`, `layouts`, `gep`, `boxing` | LLVM backend tracing |
| `SEEN_TRACE_LEXER` | `1` | Lexer operation tracing |
| `SEEN_TRACE_PARSER` | `1` | Parser operation tracing |
| `SEEN_TRACE_CODEGEN` | `1` | Code generation tracing |
| `SEEN_TRACE_ALL` | `1` | Enable all tracing |
| `SEEN_TRACE_VERBOSE` | `1` | Extra verbose trace output |
| `SEEN_LLVM_BIN` | path to LLVM `bin` dir | Override LLVM tool lookup for `opt`, `llc`, and `llvm-link` when they are not on `PATH` |

### Tracing Examples

```bash
# Debug type checking
SEEN_DEBUG_TYPES=1 seen build program.seen

# Debug all LLVM IR generation
SEEN_TRACE_LLVM=all seen build program.seen

# Debug struct field access (GEP operations)
SEEN_TRACE_LLVM=gep seen build program.seen

# Debug boxing/unboxing for generics
SEEN_TRACE_LLVM=boxing seen build program.seen
```
