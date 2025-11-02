# Seen Language MVP Development Plan (Reality‑Based, Rust‑Only)

This plan replaces previous claims with the current, verifiable status and defines the remaining work to reach a solid, self‑hosted MVP.

## Executive Summary
- Current base: Rust workspace (lexer, parser, typechecker, IR, CLI, etc.).
- CLI works for `check`, `run` (interpreter), and `build` (IR text). LLVM backend (via inkwell/LLVM 15) builds native binaries behind a feature flag.
- Stage‑1 bootstrap (LLVM): `compiler_seen/src/main.seen` now type‑checks. Native build reaches LLVM lowering and fails on a remaining unknown call target (CompileSeenProgram) and a few value‑flow edges. No C backend remains.
- Roadmap claims (RISC‑V, “100% tests”, full LSP) are not substantiated and are out of scope for the MVP.

## What’s Implemented
- CLI commands: `build`, `run`, `check`, `ir`, `repl`, `format`, `test`, `parse`, `lex`.
- IR generation and LLVM backend (feature‑gated); string helpers and method‑style lowering (length/size/endsWith/substring and string `+`).
- Parser + bundler: `import …` supported; self‑host entry no longer imports `main_compiler`.
- Docs: `docs/quickstart.md` and `docs/SELF_HOSTING_PLAN.md` updated with LLVM 15 flow.

## Gaps to MVP
- Import bundling: in place; symbol‑lists/aliasing can be refined later.
- Type + codegen: strings/arrays mostly mapped; finish a few edges (array ops as needed by sources).
- CompileSeenProgram bridge: implement in Stage‑1 as an external compile call (write temp file, invoke `seen build --backend llvm`) and later replace with in‑process pipeline.
- Determinism: Stage‑2/Stage‑3 not yet built as native; IR‑text determinism exists.
- Tests: add focused coverage for bundling, strings/arrays, and LLVM e2e.
- LSP completeness not validated; out of MVP.

## MVP Scope (revised)
Deliver a self‑hosted compiler that:
- Compiles `compiler_seen/src/main.seen` to a native binary with LLVM (no C, no stubs) — Stage‑1.
- Builds Stage‑2 and Stage‑3 from Stage‑1 with matching hashes.
- Supports minimal features used by bootstrap: functions, strings, arrays (basic), control flow, imports, simple structs.

## Plan & Milestones
- Stage‑1 (LLVM, no stubs)
  - Implement `CompileSeenProgram(source, output)` bridge: Stage‑1 writes `source` to a temp file and invokes `seen build <temp> --backend llvm --output <output>` via `__ExecuteCommand`.
  - Finalize intrinsic/call mapping (println, endsWith, substring, strlen/concat are done; fill any missing used by sources).
  - Acceptance: `cargo build -p seen_cli --release --features llvm`; `seen build compiler_seen/src/main.seen --backend llvm --output stage1_seen` succeeds.
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
- [ ] Stage‑1 native build links and runs (CompileSeenProgram bridged)
- [ ] Stage‑2/Stage‑3 deterministic (native)
- [ ] Tests for bootstrap surface and LLVM e2e

## Next 3 Tasks (tomorrow)
- Bridge CompileSeenProgram in LLVM path (call `seen build` with temp file) so Stage‑1 produces `stage1_seen`. Files: compiler_seen/src/main.seen (call site), seen_ir/src/llvm_backend.rs (intrinsic dispatch for "CompileSeenProgram").
- Confirm all runtime intrinsics used in `main.seen` are lowered (println, __Read/WriteFile, __ExecuteCommand, __GetTimestamp, string ops). File: seen_ir/src/llvm_backend.rs.
- Run `scripts/self_host_llvm.sh` and verify Stage‑2/Stage‑3 hashes match; if not, capture diffs and update tests.
