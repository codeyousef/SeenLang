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
  `@using`, `@operator`, nullable/nullish forms, `when`, closures, sealed
  classes, traits/interfaces, module namespace aliases, facade `component`
  functions, named arguments, trailing/named slot blocks, UI `state` /
  `computed` / `uiEffect` constructs, and hot-reload-facing shared-module
  patterns.

### Type Checker

Location: `compiler_seen/src/typechecker/`

- Runs frontend type validation and diagnostics.
- Tracks scoped symbols, nullable information, deterministic-mode checks, and
  effect/capability requirements.
- Emits conservative warning diagnostics for unreachable statements, unused
  locals, unused parameters, unused private top-level functions, unused import
  symbols, and unused whole-module imports.

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

## Incremental and Parallel Compilation

The compiler uses source-level and IR-level caches:

- `.seen_cache/`
- `/tmp/seen_ir_cache`
- `/tmp/seen_thinlto_cache`

Cache-v4 keys use stable module identities rather than temporary bootstrap
overlay paths. Source/object reuse is scoped by the compiler ABI signature,
project declaration hash, module body hash, LLVM tool versions, target/profile
settings, LTO/PIC/sanitizer/PGO flags, and runtime payload signatures. Body-only
edits should miss the changed module's object key without flushing otherwise
valid neighboring cache entries.

Normal multi-module compiler builds use bounded worker pools for IR generation
and optimizer work. Guarded scripts derive `SEEN_JOBS` and `SEEN_OPT_JOBS` from
memory caps and CPU count; the compiler also accepts `--jobs <n>` and
`--opt-jobs <n>`. Low-memory and bootstrap verification paths can still force
serial execution with `--no-fork`; guarded scripts also export
`SEEN_MEMORY_LIMIT_BYTES` so runtime allocation-heavy compiler phases fail with
Seen diagnostics instead of depending on host OOM behavior.

Release builds keep the full merged-IR LTO path by default for performance.
Memory-constrained callers can pass `--no-merged-release-lto` to stay on the
bounded per-module ThinLTO path.

`SEEN_TRACE_BUILD=<path>` writes JSONL build events
from rebuild scripts and compiler phases such as module discovery, declaration
scan, cache hashing, IR/object emission, runtime object reuse, release merge,
and link. `SEEN_BUILD_TRACE=<path>` remains a compatibility alias.

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
| Data enum | allocated payload/tag representation |

## Contributing to the Compiler

1. Make source changes.
2. Run source-only gates first.
3. Run `scripts/safe_rebuild.sh` only with explicit memory limits derived from
   current system memory.
4. Commit only after the relevant checks pass.

See [Bootstrap System](bootstrap.md) for the staged rebuild workflow.
