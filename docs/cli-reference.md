# CLI Reference

This page documents the shipped Seen 0.8.3 compiler binary. The release
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
| `--no-fork` | Disable parallel IR/optimization steps |
| `--projectprefix <n>` | Large-project validation prefix hint |

Supported target platforms in the shipped help are:

```text
linux-x86_64, linux-arm64, windows-x86_64,
macos-x86_64, macos-arm64, ios-arm64,
ios-sim-arm64, android-arm64
```

Examples:

```bash
seen compile hello.seen hello
seen compile app.seen app --target=linux-arm64 --target-cpu=x86-64
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
seen check <input.seen> [--profile deterministic]
```

### `seen run`

Compile and execute a Seen source file.

```bash
seen run <input.seen> [--aot] [--verbose] [--language <lang>]
```

By default `seen run` uses the JIT path. Pass `--aot` to compile an executable
first, and `--verbose` to show compiler diagnostics during the run.

### Packaging Commands

```bash
seen pkg fetch [project-dir-or-manifest]
seen pkg pack [project-dir-or-manifest] [output]
seen pkg prebuild [project-dir-or-manifest] [output-dir]
seen pkg publish <registry-dir> [project-dir-or-manifest]
```

- `fetch` installs exact-version registry dependencies into `.seen/packages/`.
- `pack` creates a source archive for the current package.
- `prebuild` emits a local prebuilt artifact containing `Seen.pkg.toml`,
  `objects.tsv`, `interface.index.tsv`, object files, and interface sources.
- `publish` writes a static-registry index and archive into a local directory.

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

- `.seen_cache/` -- source-level incremental cache
- `/tmp/seen_ir_cache` -- IR content-addressed cache
- `/tmp/seen_thinlto_cache` -- ThinLTO linker cache
