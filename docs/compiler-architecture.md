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
  classes, traits/interfaces, and hot-reload-facing shared-module patterns.

### Type Checker

Location: `compiler_seen/src/typechecker/`

- Runs frontend type validation and diagnostics.
- Tracks scoped symbols, nullable information, deterministic-mode checks, and
  effect/capability requirements.

### Bootstrap Frontend

Location: `compiler_seen/src/bootstrap/`

The bootstrap frontend wraps lexing, parsing, and type checking into the
compatibility entrypoints used by Stage 1, Stage 2, the LSP, and package
declaration scanning.

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

Normal compiler builds may use fork-parallel module work. Low-memory and
bootstrap verification paths can disable parallelism with `--no-fork` and use
the guarded scripts described in [Bootstrap System](bootstrap.md).

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
