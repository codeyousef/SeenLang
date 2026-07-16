# Project Configuration

Seen projects use `Seen.toml` for configuration.

## Minimal Seen.toml

```toml
manifest-version = 1

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

`modules` may also be written under `[build]` when you want the build entry and
the build graph in the same section. The compiler treats `[project].modules` and
`[build].modules` as one ordered module list.

```toml
[build]
entry = "src/main.seen"
modules = [
    "src/mesh_data.seen",
    "src/main.seen",
]
```

## [dependencies] Section

Seen 0.10.0 uses package aliases from `[dependencies]` as local import roots.
Registry identity, registry origin, version requirement, and import alias remain
separate values.

```toml
manifest-version = 1

[registries]
default = "https://seen.dev.yousef.codes/packages"

[dependencies]
calc = { package = "alice/mathx", version = "^1.2.0", allow = ["file"] }
web = { package = "seen/web", version = "~2.4", allow = ["network"] }
gamekit = { path = "../gamekit" }

[package-grants]
"alice/mathx" = ["file"]
"seen/web" = ["network"]
```

`manifest-version = 1` is required for registry dependencies and publishable
package fields. Registry origins are canonical absolute HTTPS URLs. The
development URL above is the live official origin, and the client embeds its
official trust root for public signed metadata and catalog reads. Production
will later use `https://seen.yousef.codes/packages` with independent environment
routing and signing configuration; it is not deployed and has no embedded
root. Internal development submissions remain delayed and unavailable, so they
cannot satisfy dependency resolution until promotion is implemented.

For a custom HTTPS registry, its first manual `seen pkg fetch` also supplies
`--environment <alias>=development|production` and
`--repository-id <alias>=seen-dev-...|seen-prod-...` alongside the trusted-root
path and SHA-256 pin. These values are checked against the signed root and then
recovered from verified private trusted state on later compiler-triggered
fetches; they do not need to be copied into `Seen.toml`. A conflicting value is
never allowed to replace the pinned identity. See [CLI Reference](cli-reference.md#packaging-commands)
for the complete bootstrap example.

Package dependencies can be either:

- scoped registry packages like
  `{ package = "alice/mathx", version = "^1.2.0", allow = ["file"] }`; the
  table key is the local import alias
- local Seen package paths like `{ path = "../gamekit" }`
- local prebuilt artifacts like `{ artifact = "../dist/gamekit-0.1.0.seenpkg" }`

Registry requirements support exact versions, caret requirements such as
`^2.1.0`, tilde requirements such as `~1.4`, and bounded comparator
conjunctions such as `>=1.2.3 <2.0.0`. Wildcards, tags, OR expressions, and
unbounded comparator forms are rejected so every accepted requirement has one
canonical meaning.

Dependencies are imported by the local dependency key, independent of their
canonical registry identity:

```seen
import calc.value.{answer}
import gamekit.player.{Player}
```

`allow` is the publisher-edge consent for the dependency's signed capability
request. `[package-grants]` records the root project's consent by canonical
identity and must cover every direct and transitive package. Supported
capabilities are `file`, `network`, `process`, `environment`, `dynamic-load`,
`ffi`, `unsafe`, `native-link`, and `macro`. These declarations are policy and
consent signals, not an operating-system sandbox.

Registry packages are stored in immutable content-addressed cache entries and
exposed through project-local read-only views only after the complete graph,
metadata, archive digests, and capability grants verify. Registry-backed
projects get an atomically written dependency `Seen.lock` version 2 recording
the manifest digest, root edges, all reachable transitive nodes, canonical
origins, exact versions and archive digests, signed target paths and metadata
versions, dependency edges, capability requests, and grants. The compiler
enforces that lock in `--locked` mode; `--offline` prohibits network access and
uses verified local state; `--frozen` applies both restrictions. The stdlib
ABI/module snapshot is a separate `Seen.modules.lock` artifact.

## [package] Section

A publishable package keeps its local module root separate from its registry
identity:

```toml
[project]
name = "math_core"
version = "0.1.0"

[package]
identity = "alice/mathx"
visibility = "public"
include = ["src/**/*.seen", "README.md", "LICENSE"]
assets = []
capabilities = []
```

`project.name`, a consumer's dependency alias, and `package.identity` are not
required to match. Hosted archives are source-only and are checked against the
package's declared include/assets lists and signed metadata before installation.

## [native.dependencies] Section

```toml
[native.dependencies]
sdl3 = { path = "native/lib" }
vulkan = {}
```

`[native.dependencies]` controls linker-facing native libraries. For
project-local native libraries, add `path = "..."` to point at the directory
containing the library file. The path is resolved relative to the nearest
`Seen.toml`. Seen adds `-L<resolved-path>` during linking, and on native
Linux/macOS builds it also records that directory as a runtime search path so
the output can run without extra `LIBRARY_PATH` or `LD_LIBRARY_PATH` wrappers.

Legacy `system = true` entries inside `[dependencies]` are still accepted for
backward compatibility, but new manifests should prefer `[native.dependencies]`.

## [build] Section

The manifest may include build metadata for projects and tools, but the shipped
compiler's user-facing build controls are CLI flags documented in
[CLI Reference](cli-reference.md).

```toml
[build]
targets = ["native", "linux-x86_64", "linux-riscv64"]
optimize = "speed"      # "speed" or "size"
lto = true              # Link-time optimization
codegen-units = 1       # Single unit for best optimization
debug-info = true       # Include debug symbols
profile = "release"     # "release" or "debug"
```

## [targets.*] Section

Target sections are project metadata for tools and future target profiles. Use
`seen compile --target=<platform>` for the shipped compiler's target selection.
Canonical target names and triples are listed in [Compilation Targets](targets.md).

Per-target configuration:

```toml
[targets.native]
triple = "x86_64-unknown-linux-gnu"
features = ["simd", "vectorization"]

[targets.linux-riscv64]
triple = "riscv64-unknown-linux-gnu"
features = ["rv64gc", "compressed", "atomic"]
```

## [format] Section

Format settings are consumed by editor/tooling integrations. The shipped
compiler binary does not expose a standalone formatter command.

Code formatting preferences:

```toml
[format]
line-width = 100
indent = 4
trailing-comma = true
document-types = [".seen", ".md", ".toml"]
```

## [test] Section

Test settings are reserved for project tooling. Use repository test scripts or
package-specific test runners where provided.

```toml
[test]
threads = "auto"    # number of test threads
timeout = 300       # seconds per test
coverage = true     # enable code coverage
```

## [benchmark] Section

Benchmark settings are reserved for project tooling and benchmark scripts.

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
- [Packaging](packaging.md) -- package registries and publishing
- [CLI Reference](cli-reference.md) -- build commands
