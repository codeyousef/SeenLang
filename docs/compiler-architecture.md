# Compiler Architecture

The Seen compiler is self-hosted and compiles through LLVM for native code
generation. The shipped release binary uses `compiler_seen/src/main_compiler.seen`
as its command entrypoint.

## Pipeline Overview

```text
Source (.seen)
  -> Lexer
  -> Parser
  -> Type checker
  -> Multi-module LLVM IR generation
  -> opt/llc or target compiler tools
  -> Native binary or target artifact
```

## Frontend

### Lexer

Location: `compiler_seen/src/lexer/`

- Loads keyword/operator tables from `languages/<lang>/`.
- Preserves source locations for diagnostics.
- Supports line comments and standalone-delimited `/// ... ///` block comments.
- Emits language-neutral token types so later stages do not need to know which
  human language was used.

### Parser

Location: `compiler_seen/src/parser/`

- Recursive-descent parser centered on `real_parser.seen`.
- Produces program, declaration, statement, and expression nodes.
- Parses current syntax including package imports, `effect(Token)`,
  `@using`, `fun operator+` declarations, nullable/nullish forms, `when`, closures, sealed
  classes, traits/interfaces, module namespace aliases, facade `component`
  functions, named arguments, trailing/named slot blocks, UI `state` /
  `computed` / `uiEffect` constructs, and hot-reload-facing shared-module
  patterns.

### Type Checker

Location: `compiler_seen/src/typechecker/`

Type validation is split across the bootstrap/frontend path, focused
typechecker modules, and whole-program checks in `main_compiler.seen`; there is
not one monolithic authoritative pass. The `TypeChecker` interface module also
serves compatibility callers. Together these paths track scoped symbols,
nullable information, deterministic-mode checks, effect/capability
requirements, and conservative unused/unreachable warnings.

### Bootstrap Frontend

Location: `compiler_seen/src/bootstrap/`

The bootstrap frontend wraps lexing, parsing, and type checking into the
compatibility entrypoints used by Stage 1, Stage 2, the LSP, and package
declaration scanning.

New bootstrap helper modules must be reachable from `main_compiler.seen`
imports as well as from the embedded compiler-module list. That keeps older
bootstrap compilers from treating new helper calls as external declarations with
the wrong ABI during self-hosted rebuilds.

## Code Generation

Location: `compiler_seen/src/codegen/`

The LLVM generator is now split into focused driver and helper modules rather
than a single monolithic implementation. `llvm_ir_gen.seen` is the public facade;
state-based helpers handle declarations, modules, functions, calls, binary
expressions, method calls, statements, literals, member/index access, control
flow, runtime declarations, and target-specific state.

Generation is organized around:

1. Declaration/signature collection.
2. Type/layout and registry preparation.
3. Function and module body lowering to LLVM IR.
4. Object emission, optimization, and linking.

Package artifacts participate in code generation through interface indexes and
object manifests: dependency declarations are scanned, provided modules are
skipped for codegen, and prebuilt objects are linked into the final binary.

### Refactored Codegen Layout

The refactor intentionally leaves `llvm_ir_gen.seen` boring. It owns the
compatibility API, bridges legacy facade fields into shared state, and delegates
real lowering work to smaller modules. A quick rule of thumb:

| Module family | What belongs there |
|---------------|--------------------|
| `ir_decl_*` | declaration scanning, runtime declarations, type registration |
| `ir_module_*` | module entry/tail emission, string constants, object-unit flow |
| `ir_function_*` | function identity, attributes, entry/exit state, body setup |
| `ir_call_*` and `ir_method_*` | call planning, receiver handling, argument lowering |
| `ir_stmt_*` and `ir_*_driver` | statement/expression orchestration |
| `ir_*_emit` and `ir_*_plan` | leaf emission and small planning decisions |

Comments in these files should explain the boundary or invariant, not restate
the line of code below them. Good comments answer questions such as "why is this
state copied here?", "why does this pass run before that one?", or "what must be
true when this helper returns?".

## Backend and Targets

The shipped compiler supports the LLVM backend. It can emit native binaries and
target artifacts for the platforms listed in [CLI Reference](cli-reference.md).
Important target controls include `--target`, `--target-cpu`, `--simd`,
`--sanitize`, `--pgo-generate`, `--pgo-use`, `--pic`, and
`--object-manifest`.

## Native-boundary ledger

Native ABI use is explicit and versioned in
[`architecture/native-boundaries.json`](architecture/native-boundaries.json).
It records the owning subsystem, purpose, ABI, supported platforms, and each
foreign symbol. The ledger is a fail-closed contract: update it alongside any
production FFI addition, then run `tests/misc_root_tests/seen_native_boundaries_ledger.sh`.
The JSON shape is defined by
[`../schemas/native-boundaries.schema.json`](../schemas/native-boundaries.schema.json).

[`architecture/native-inventory.json`](architecture/native-inventory.json)
is the deterministic source inventory behind that review contract. It records
every production Seen `extern fun` symbol with its declaring source files, all
backend implementations present in compiler source, and which backend the
shipped CLI exposes. `scripts/ci_required.sh` regenerates the inventory in
memory and rejects any byte-level drift before a pull request can merge.

## Required CI contract

Gate 0 has one active workflow and one required job: `.github/workflows/ci.yml`
publishes `CI / required` from an Ubuntu 24.04 runner. It uses a commit-pinned
checkout action, read-only repository permissions, a ten-minute timeout, and
the fail-closed `scripts/run_ci_required.sh` entry point. That outer entry point
derives the aggregate cap from live system memory, enters a Linux cgroup v2
user-systemd scope, and applies zero swap, serial workers, a 24-task ceiling,
per-process virtual-memory limits, and a 540-second child timeout. The inner
`scripts/ci_required.sh` gate re-reads the live kernel scope before running the
deterministic policy and containment regressions; an environment marker alone
is never accepted as evidence.

The aggregate and main-compiler limits are the smaller of 25% of total memory
and 50% of currently available memory. They have no fixed byte ceiling; this
keeps the same bounded host fraction on both small CI runners and larger
development machines. Caller-supplied limits are accepted only when they are
no larger than the freshly derived value.

The exact reviewed limits and platform support are versioned in
[`architecture/ci-containment.json`](architecture/ci-containment.json), with
the JSON shape fixed by
[`../schemas/ci-containment.schema.json`](../schemas/ci-containment.schema.json).
Linux x86-64 is the required runtime lane and Linux ARM64 receives static
policy coverage. macOS and Windows are explicitly unsupported and fail closed
until an equivalent read-back-verified aggregate boundary exists.

Obsolete disabled workflows are not retained as fallbacks: they referenced
unsupported compiler commands and paths and bypassed current containment
policy. `scripts/check_ci_workflows.py` scans `.github` with explicit file and
byte bounds, rejects links and retired workflow paths, and compares the active
workflow byte-for-byte with the reviewed contract. Run
`tests/misc_root_tests/seen_ci_workflow_contract.sh` and
`tests/misc_root_tests/seen_ci_containment_contract.sh` after any CI-policy
change.

## Release compatibility contract

Every release has a strict machine-readable compatibility record at
`releases/compatibility-manifest.json`. It identifies the compiler and package
client versions, runtime and compiler ABIs, package artifact schemas, standard
library module-manifest version, minimum LLVM major, and all advertised target
triples. The JSON contract is defined by
`schemas/compatibility-manifest.schema.json` and rendered deterministically by
`scripts/check_compatibility_manifest.py`.

`compiler_seen/src/release/compatibility.seen` owns both layers of the native
contract. `validateCompatibilityManifest` retains the bounded, side-effect-free
`core.002a.*` schema checks. `generateCompatibilityManifest` accepts an
explicit `CompatibilityReleaseInputs`, and `renderCompatibilityManifest`
produces one canonical UTF-8 representation. The strict decoder rejects
unknown and duplicate fields before constructing the typed model;
`consumeCompatibilityManifest` compares the canonical decoded record with the
complete runtime expectation. Atomic output validates before writing and uses
the runtime's transactional replacement primitive, so cancellation and errors
leave no partial manifest.

Before `seen pkg` launches the version-coupled package client, the compiler
consumes `releases/compatibility-manifest.json` in a source checkout or the
same installer-shipped bytes beside the executable. The manifest supplies the
sidecar version and request protocol only after every compiler, runtime,
standard-library, ABI, platform, and target value matches. There is no PATH,
default-value, or partial-manifest fallback.

The manifest's `seen-package-interface-v2` component entry binds the
independently versioned `seen-package-layout-v1` compatibility identity, which
the same native module owns with its bounded `ReusablePackageLayout`
validation/rendering API.
Every path and platform claim is supplied explicitly. The API accepts only the
canonical `Seen.toml`, `src/mod.seen`, tests, examples, readme, and license
mapping and returns typed `pkg.layout.001.*` errors instead of normalizing a
different tree.

## Incremental and Parallel Compilation

The compiler uses source-level and IR-level caches:

- `.seen_cache/`
- `<SEEN_ARTIFACT_ROOT>/seen_ir_cache/`
- `<SEEN_ARTIFACT_ROOT>/seen_thinlto_cache/`
- `target/seen-build/runtime-objects/`
- `target/seen-build/release-lto/`

Cache-v4 keys use stable module identities rather than temporary bootstrap
overlay paths. Source/object reuse is scoped by the compiler binary hash,
compiler ABI signature, project declaration hash, module body hash, LLVM tool
versions, target/profile settings, LTO/PIC/sanitizer/PGO flags, and runtime
payload signatures. Body-only edits should miss the changed module's object key
without flushing otherwise valid neighboring cache entries, while compiler
codegen/layout changes reject stale objects automatically.

Normal multi-module compiler builds use bounded worker pools for IR generation
and optimizer work. Guarded scripts derive `SEEN_JOBS` and `SEEN_OPT_JOBS` from
memory caps and CPU count; the compiler also accepts `--jobs <n>` and
`--opt-jobs <n>`. Low-memory and bootstrap verification paths can still force
serial execution with `--no-fork`; guarded scripts also export
`SEEN_MEMORY_LIMIT_BYTES` so runtime allocation-heavy compiler phases fail with
Seen diagnostics instead of depending on host OOM behavior.

Release builds keep the full merged-IR LTO path by default for performance.
Memory-constrained callers can pass `--no-merged-release-lto` to stay on the
bounded per-module ThinLTO path. Warm release builds can reuse a
signature-keyed merged-LTO object while preserving the default merged-LTO mode.

`seen compile --emit-module-ir-dir <dir> --stop-after-ir` writes raw per-module
LLVM IR into a caller-owned directory and exits before object emission/linking.
Packaging and cross-build scripts use this instead of scraping compiler-owned
scratch artifacts. By default, compiler scratch and these caches live below
the checkout's ignored `.seen/agent-tools/compiler/` directory.

`SEEN_TRACE_BUILD=<path>` writes JSONL build events
from rebuild scripts and compiler phases such as module discovery, declaration
scan, cache hashing, IR/object emission, runtime object reuse, release merge,
release-LTO mode, and link. `SEEN_BUILD_TRACE=<path>` remains a compatibility
alias. Compiler trace events use millisecond timestamps and escaped JSON fields.

## Key Source Areas

| Area | Purpose |
|------|---------|
| `compiler_seen/src/main_compiler.seen` | Shipped compiler CLI and bootstrap driver |
| `compiler_seen/src/main.seen` | Higher-level CLI wrapper source, not the current release entrypoint |
| `compiler_seen/src/bootstrap/` | Frontend orchestration and diagnostic compatibility |
| `compiler_seen/src/lexer/` | Tokenization and multilingual keyword loading |
| `compiler_seen/src/parser/` | AST construction |
| `compiler_seen/src/typechecker/` | Type, effect, and deterministic-mode checks |
| `compiler_seen/src/codegen/` | LLVM IR generation, runtime declarations, backend helpers |
| `seen_std/src/` | Standard library modules |
| `seen_runtime/` | C runtime primitives linked by Seen programs |

## Type Representation in LLVM IR

| Seen Type | LLVM IR Shape |
|-----------|---------------|
| `Int` | `i64` |
| `Float` | `double` |
| `Bool` | `i1` |
| `String` | `%SeenString` (`{ i64, ptr }`) |
| `Char` | `i64` |
| `Array<T>` | runtime array handle/pointer |
| Class/value handles | pointer or handle depending on lowering path |
| Simple enum | integer tag |
| Payload/data enum | experimental payload/tag paths; verify each use with a focused test |

## Contributing to the Compiler

1. Make source changes.
2. Run source-only gates first.
3. Run `scripts/safe_rebuild.sh` only with explicit memory limits derived from
   current system memory.
4. Commit only after the relevant checks pass.

See [Bootstrap System](bootstrap.md) for the staged rebuild workflow.
