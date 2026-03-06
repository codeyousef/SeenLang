# Project Configuration

Seen projects use `Seen.toml` for configuration.

## Minimal Seen.toml

```toml
[project]
name = "my_project"
version = "0.1.0"
language = "en"
```

## Project Structure

```
my_project/
├── Seen.toml
├── src/
│   └── main.seen
├── tests/
│   └── test_main.seen
└── benchmarks/
    └── bench_main.seen
```

## [project] Section

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Project name |
| `version` | String | Yes | Semantic version (e.g., `"1.0.0"`) |
| `language` | String | No | Keyword language: `en`, `ar`, `es`, `ru`, `zh`, `ja` (default: `en`) |
| `visibility` | String | No | Visibility model: `"caps"` (capability-based) |
| `description` | String | No | Project description |
| `authors` | Array | No | List of author names |
| `edition` | String | No | Language edition (e.g., `"2025"`) |
| `modules` | Array | No | Explicit module list |

Example:

```toml
[project]
name = "seen_compiler"
version = "1.0.0"
language = "en"
visibility = "caps"
description = "Self-hosted Seen compiler"
authors = ["Seen Language Team"]
edition = "2025"
modules = [
    "src/bootstrap",
    "src/lexer",
    "src/parser",
    "src/typechecker",
    "src/main.seen",
]
```

## [dependencies] Section

```toml
[dependencies]
seen_std = "../seen_std"
```

Dependencies are specified as relative paths to other Seen projects.

## [build] Section

```toml
[build]
targets = ["native", "wasm32", "riscv64"]
optimize = "speed"      # "speed" or "size"
lto = true              # Link-time optimization
codegen-units = 1       # Single unit for best optimization
debug-info = true       # Include debug symbols
profile = "release"     # "release" or "debug"
```

## [targets.*] Section

Per-target configuration:

```toml
[targets.native]
triple = "x86_64-unknown-linux-gnu"
features = ["simd", "vectorization"]

[targets.riscv64]
triple = "riscv64-unknown-linux-gnu"
features = ["rvv", "compressed", "atomic"]

[targets.wasm32]
triple = "wasm32-unknown-unknown"
features = ["simd128"]
```

## [format] Section

Code formatting preferences:

```toml
[format]
line-width = 100
indent = 4
trailing-comma = true
document-types = [".seen", ".md", ".toml"]
```

## [test] Section

```toml
[test]
threads = "auto"    # number of test threads
timeout = 300       # seconds per test
coverage = true     # enable code coverage
```

## [benchmark] Section

```toml
[benchmark]
iterations = 1000
warmup = 100
timeout = 60
statistical-significance = 0.05
```

## [lsp] Section

Language server features:

```toml
[lsp]
diagnostics = true
completion = true
hover = true
goto-definition = true
find-references = true
semantic-tokens = true
```

## [performance] Section

Performance targets (informational):

```toml
[performance]
lexer-throughput = "25M tokens/sec"
parser-throughput = "800K lines/sec"
typechecker-speed = "80μs/function"
codegen-speed = "300μs/function"
memory-overhead = "10%"
```

## Related

- [Getting Started](getting-started.md) -- project setup
- [CLI Reference](cli-reference.md) -- build commands
