# CLI Reference

This page documents the shipped Seen 0.10.0 compiler binary. The release
entrypoint is `seen compile`; older `seen build` examples are stale for the
current packaged compiler.

## Commands

### Global flags

```bash
seen --version
seen -v
seen --help
seen -h
```

`--version` prints the shipped compiler version and exits successfully.
`--help` prints the supported command surface and exits successfully.

### `seen compile`

Compile a Seen source file to a native binary or target artifact.

```bash
seen compile <input.seen> [output] [options]
```

Common options:

| Option | Description |
|--------|-------------|
| `--profile deterministic` | Reject nondeterministic collection usage unless explicitly annotated |
| `--no-cache` | Disable incremental compilation caching |
| `--verbose` | Show full per-module compiler progress and expanded warning diagnostics |
| `--locked` | Require the existing `Seen.lock` to match `Seen.toml`; do not rewrite it |
| `--offline` | Prohibit package-network access and use only verified local metadata and blobs |
| `--frozen` | Apply both `--locked` and `--offline` |
| `--language <lang>` / `-l <lang>` | Source keyword language: `en`, `ar`, `es`, `ru`, `zh`, `ja` |
| `--target=<platform>` / `--target <platform>` | Cross-compile target |
| `--target-cpu=<cpu>` | CPU baseline: `native`, `x86-64`, `x86-64-v3`, `x86-64-v4` |
| `--simd=<policy>` | SIMD policy: `auto`, `none`, `sse4.2`, `avx2`, `avx512` |
| `--simd-report` / `--simd-report=full` | Show LLVM vectorization reports |
| `--backend=<name>` / `--backend <name>` | Backend selector; the shipped binary supports LLVM |
| `--deterministic` | Deterministic mode with scalar SIMD and no unchecked `HashMap` |
| `--sanitize=<policy>` | Sanitizer policy: `address`, `undefined`, `thread`, `memory` |
| `--pgo-generate` | Build with profile-generation instrumentation |
| `--pgo-use=<path>` | Use a merged PGO profile |
| `--ml-log=<path>` | Collect optimization training data |
| `--ml-decision-log=<path>` | Write optimization decision logs as JSONL |
| `--pic` | Emit PIC objects suitable for shared-library links |
| `--object-manifest <path>` | Write an object-to-module TSV manifest and skip final executable link |
| `--fast` | Use the lightweight optimization path used by bootstrap verification |
| `--no-merged-release-lto` | Disable full merged release LTO for lower-memory release builds |
| `--emit-module-ir-dir <dir>` | Emit raw per-module LLVM IR into `<dir>` for packaging/cross-build tools |
| `--stop-after-ir` | Stop after `--emit-module-ir-dir`; requires an IR output directory |
| `--no-fork` | Disable parallel IR/optimization steps |
| `--projectprefix <n>` | Large-project validation prefix hint |

Supported target platforms in the shipped help are:

```text
linux-x86_64, linux-arm64, linux-riscv64,
windows-x86_64, macos-x86_64, macos-arm64,
ios-arm64, ios-sim-arm64, android-arm64
```

See [Compilation Targets](targets.md) for target triples, aliases, and
RISC-V/QEMU verification commands.

By default, `seen compile` prints bounded progress at useful phase checkpoints
instead of one line per internal action. Warning diagnostics remain visible, and
`--verbose` expands progress and warnings when debugging a specific module.

Examples:

```bash
seen compile hello.seen hello
seen compile app.seen app --target=linux-arm64 --target-cpu=x86-64
seen compile app.seen app-rv64 --target=linux-riscv64
seen compile plugin.seen plugin_host --pic --no-cache --no-fork \
  --object-manifest /tmp/plugin.objects.tsv
```

When `--object-manifest` is present, `seen compile` stops after object emission
and records one tab-separated row per emitted module object:

```text
/tmp/seen_module_0.o	src/plugin.seen
```

### `seen check`

Run frontend/type checks without building an executable.

```bash
seen check <input.seen> [--profile deterministic] [--locked|--offline|--frozen]
```

Package resolution modes have the same meaning as for `seen compile`.

### `seen run`

Compile and execute a Seen source file.

```bash
seen run <input.seen> [--aot] [--no-cache] [--verbose] [--language <lang>] [--locked|--offline|--frozen]
```

By default `seen run` uses the JIT path. Pass `--aot` to compile an executable
first, `--no-cache` to force a fresh compile, and `--verbose` to show compiler
diagnostics during the run. Run flags may appear before or after the input path.

### Packaging Commands

```bash
seen pkg add|remove|fetch|update [options]
seen pkg tree [--lock <Seen.lock>]
seen pkg audit [--lock <Seen.lock>]
seen pkg pack [options]
seen pkg publish [project-dir-or-manifest] [--registry <origin>] [--token-file <mode-0600-file>] [--source-forge github|gitlab] [source options]
seen pkg prebuild [project-dir-or-manifest] [output-dir]
```

- `add` and `remove` edit dependencies in `Seen.toml`.
- `fetch` resolves the complete dependency graph, verifies signed metadata and
  archives, installs read-only project views, and atomically writes `Seen.lock`.
- `update` ignores lock preference and selects the newest eligible graph.
- `tree` prints a canonical lock graph; `audit` validates the lock graph and
  capability bindings and lists the locked package digests. Both accept an
  explicit lock path.
- `pack` creates a validated source archive for the current package.
- `publish` submits a source package with an authorized internal credential and
  bound source forge, repository, installation, ref, commit, and SPDX license
  metadata. Development submissions complete as quarantined and unavailable;
  the public delay begins only after source verification and the first scan pass.
- `prebuild` emits a local prebuilt artifact containing `Seen.pkg.toml`,
  `objects.tsv`, `interface.index.tsv`, object files, and interface sources.

`fetch` accepts `--locked`, `--offline`, and `--frozen`. Normal mode prefers a
valid locked candidate and may update the lock. `--locked` requires the existing
lock and never changes it; `--offline` permits only unexpired, previously
verified local metadata and blobs; `--frozen` applies both. `update` cannot be
combined with `--locked` or `--frozen`.

The official development registry uses its embedded root and needs no manual
`--trusted-root` flags. The first fetch from a custom signed registry must
establish its out-of-band root and immutable signing identity:

```bash
seen pkg fetch \
  --trusted-root custom=/secure/custom.root.json \
  --trusted-root-sha256 custom=<sha256> \
  --environment custom=development \
  --repository-id custom=seen-dev-custom-v1
```

The alias must match the key under `[registries]`. After the pinned root is
verified, its signed `environment` and `repository_id` are retained in private
trusted state. Later `fetch` calls—and automatic fetches issued by
`compile`, `check`, or `run`—need only the manifest and resolution mode.
Supplying an environment or repository ID that conflicts with the trusted root
is rejected. Deleting the package metadata cache intentionally removes this
local trust state and makes a new explicit pinned-root bootstrap necessary for
custom origins; the official development root remains available from the
client.

Controlled internal publishing is available through:

```bash
seen pkg publish [project-dir-or-manifest] \
  --token-file <mode-0600-file> \
  --source-forge github \
  --source-repository-id <id> \
  --source-installation-id <id> \
  --source-ref refs/heads/<branch> \
  --source-commit <full-commit> \
  --license-spdx <identifier>
```

`--source-forge` accepts exactly `github` or `gitlab`. The equivalent
`SEEN_SOURCE_FORGE` environment variable has the same validation and defaults
to `github` when neither form is supplied.

On Linux and macOS, a token file must be one private regular file and must not
be selected by the package's `include` or `assets` patterns. Windows rejects
`--token-file`; inject `SEEN_REGISTRY_TOKEN` through the trusted publisher
process environment instead.

The development service accepts the bound submission but keeps the release
delayed, unavailable, and excluded from public catalog, resolution, and
download. The CLI still reserves these inactive hosted operations:

```bash
seen pkg login|logout|whoami [options]
seen pkg yank|report [options]
```

They and private-package access remain inactive in 0.10.0. The development read
service and embedded trust root are live; production remains absent and fails
closed.

### Platform Packaging Commands

```bash
seen bundle <executable> <AppName> [--icon=<icon.icns>] [--version=<1.0>]
seen sign <path> [--identity=<identity>]
seen notarize <path> --apple-id=<email> --team-id=<id> --password=<pwd>
seen lipo <x86_64_binary> <arm64_binary> [--output=<universal>]
seen lipo --from-source <source.seen> [--output=<universal>]
seen ipa <executable> <AppName> [--bundle-id=...] [--version=...] [--provisioning-profile=...]
```

### Other Commands

```bash
seen translate <file> --from <lang> --to <lang> [-o <output>]
seen import-c <header.h>
seen lsp
```

`translate` rewrites source keywords between supported Seen languages.
`import-c` generates Seen `extern fun` declarations from C headers.
`lsp` starts the built-in language server.

## PGO Workflow

```bash
seen compile prog.seen prog --pgo-generate
./prog
llvm-profdata merge -o default.profdata default_*.profraw
seen compile prog.seen prog --pgo-use=default.profdata
```

## Unsupported Source-Wrapper Commands

The shipped compiler intentionally does not expose the legacy source-wrapper
commands `build`, `init`, `fmt`, `format`, `clean`, or `test`. These commands
fail with a clear diagnostic. Use `seen compile` for builds and
project-specific scripts for scaffolding, cleanup, and tests until those
surfaces are implemented in the shipped entrypoint.

The shipped compiler is LLVM-only. Passing `--backend=c` to `seen compile`
fails with an explicit unsupported-backend diagnostic.

## Cache Locations

- `.seen/views/` -- project-local read-only package views
- `.seen/package-map.tsv` -- authoritative alias-to-package-view mapping for the project
- `.seen_cache/` -- source-level incremental cache
- `/tmp/seen_ir_cache` -- IR content-addressed cache
- `/tmp/seen_thinlto_cache` -- ThinLTO linker cache
- `target/seen-build/runtime-objects/` -- signature-keyed runtime objects
- `target/seen-build/release-lto/` -- merged release-LTO object cache
- `target/seen-build/perf-baselines/` -- performance gate baselines
- `target/seen-build/package-artifacts/` -- release package artifact caches
