# Seen Language MVP Development Plan (Reality‑Based, Rust‑Only)

This plan replaces previous claims with the current, verifiable status and defines the remaining work to reach a solid, self‑hosted MVP.

## Executive Summary
- Current base: Rust workspace (lexer, parser, typechecker, IR, CLI, etc.).
- CLI works for `check`, `run` (interpreter), and `build` (IR text). LLVM backend (via inkwell/LLVM 15) produces native binaries behind a feature flag.
- Stage‑1 bootstrap (LLVM): `compiler_seen/src/main.seen` now emits LLVM IR, feeds `llc`, and links a working native `stage1_seen` binary via the Rust CLI. The bridge no longer depends on the legacy C path.
- Roadmap claims (RISC‑V, “100% tests”, full LSP) are not substantiated and are out of scope for the MVP.

## What’s Implemented
- CLI commands: `build`, `run`, `check`, `ir`, `repl`, `format`, `test`, `parse`, `lex`.
- IR generation and LLVM backend (feature‑gated); string helpers and method‑style lowering (length/size/endsWith/substring and string `+`).
- Parser + bundler: `import …` supported; self‑host entry no longer imports `main_compiler`.
- Docs: `docs/quickstart.md` and `docs/SELF_HOSTING_PLAN.md` updated with LLVM 15 flow.

## Gaps to MVP
- Import bundling: in place; symbol‑lists/aliasing can be refined later.
- Type + codegen: strings/arrays mostly mapped; finish a few edges (array ops as needed by sources).
- Stage‑2/Stage‑3 determinism: native pipeline still pending; IR‑text determinism exists.
- Tests: add focused coverage for bundling, strings/arrays, and LLVM e2e.
- LSP completeness not validated; out of MVP.

## MVP Scope (revised)
Deliver a self‑hosted compiler that:
- Compiles `compiler_seen/src/main.seen` to a native binary with LLVM (no C, no stubs) — Stage‑1.
- Builds Stage‑2 and Stage‑3 from Stage‑1 with matching hashes.
- Supports minimal features used by bootstrap: functions, strings, arrays (basic), control flow, imports, simple structs.

## Plan & Milestones
- Stage‑1 (LLVM, bridge hardened)
  - Automate Stage‑1 bootstrap steps (`seen --emit-ll`, `llc`, `cc -no-pie`) behind the CLI and capture linker diagnostics.
  - Add smoke tests that ensure Stage‑1 binary links and runs on CI images with LLVM 15 + system `cc`.
  - Acceptance: Stage‑1 build scripted (`scripts/self_host_llvm.sh`) and green in CI.
- Import/bundler
  - Support symbol‑list imports and aliasing; verify `import main_compiler.{CompileSeenProgram}`.
  - Acceptance: no manual stubs; Stage‑1 build succeeds.
- Stage‑2/Stage‑3 determinism
  - Build Stage‑2 with Stage‑1, Stage‑3 with Stage‑2; compare hashes.
  - Acceptance: `sha256(stage2_seen) == sha256(stage3_seen)`.
- Tests and docs
  - Add targeted tests: imports, triple‑quoted/interpolated strings, arrays/strings, bundler, LLVM e2e.
  - Keep quickstart and verifier instructions updated.

## Non‑Goals for MVP
- RISC‑V/ISA/vector support, LLVM performance targets, full Kotlin‑feature parity, and complete LSP. These are follow‑ups.

## Risks
- Type/codegen gaps causing LLVM type errors.
- Over/under‑bundling causing duplicates or missing symbols.
- Hidden dependencies in `compiler_seen` requiring additional runtime helpers.

## Current Reality Checklist
- [x] CLI builds and basic flows work
- [x] LLVM backend builds with LLVM 15
- [x] Self‑host entry type‑checks
- [x] Stage‑1 native build links and runs (CompileSeenProgram bridged)
- [ ] Stage‑2/Stage‑3 deterministic (native)
- [ ] Tests for bootstrap surface and LLVM e2e

## Next 3 Tasks (tomorrow)
- Promote the Stage‑1 pipeline into `scripts/self_host_llvm.sh` (CLI emit → `llc` → `cc -no-pie`), including environment checks for clang/cc.
- Use the Stage‑1 binary to build Stage‑2/Stage‑3 and capture hashes; diff artifacts on mismatch.
- Add regression tests covering the new bridge (temp cleanup, linker flag) and ensure CI exercises the LLVM feature build.
