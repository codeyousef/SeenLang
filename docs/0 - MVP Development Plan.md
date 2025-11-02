# Seen Language MVP Development Plan (Reality‑Based)

This plan replaces previous claims with the current, verifiable status and defines the remaining work to reach a solid, self‑hosted MVP.

## Executive Summary
- Current base: Rust workspace (lexer, parser, typechecker, IR, CLI, etc.).
- CLI works for `check`, `run` (interpreter), and `build` (Seen → C) on simple samples.
- Stage‑1 bootstrap: We can emit C for `compiler_seen/src/main.seen` and build a native binary, but only with injected C stubs and minimal string helpers. This is not a complete self‑host yet.
- Roadmap claims (RISC‑V, “100% tests”, full LSP) are not substantiated and are out of scope for the MVP.

## What’s Implemented
- CLI commands: `build`, `run`, `check`, `ir`, `repl`, `format`, `test`, `parse`, `lex`.
- IR generation and LLVM backend (feature‑gated); string helpers and method‑style lowering (length/size/endsWith/substring and string `+`).
- Parser handles `import …` syntax.
- Docs: `docs/quickstart.md` and `docs/SELF_HOSTING_PLAN.md` exist and are actionable.

## Gaps to MVP
- Import bundling: no real module resolution or bundling of `main_compiler` and deps; Stage‑1 succeeded only with injected stubs.
- Type + codegen integration: strings/arrays/structs not consistently typed through IR to LLVM; some temps are untyped in IR and need conservative lowering.
- Lists/arrays: construction, length, indexing, and returns need full end‑to‑end lowering + helpers.
- Built‑ins/runtime: minimal runtime for `println`, string ops, and basic IO (currently stubbed for Stage‑1).
- Determinism: Stage‑2/Stage‑3 not yet produced or hash‑compared.
- Tests: no focused tests for bootstrap surface (imports, strings, method lowering, bundling, simple end‑to‑end C build).
- LSP completeness not validated; keep out of MVP.

## MVP Scope (revised)
Deliver a self‑hosted compiler that:
- Compiles `compiler_seen/src/main.seen` to C without stubs and builds/runs (Stage‑1).
- Builds Stage‑2 and Stage‑3 from Stage‑1 with matching hashes.
- Supports the minimal feature set used by bootstrap: functions, strings, arrays (basic), control flow, imports, simple structs.

## Plan & Milestones
- Stage‑1 without stubs
  - Implement list/array lowering and helpers in C.
  - Track/emit correct C types for strings/structs; fix struct/list returns.
  - Provide minimal C runtime for built‑ins (println, file IO, exec, time, format).
- Acceptance: `cargo build -p seen_cli --release --features seen_ir/llvm`; `seen build compiler_seen/src/main.seen --backend llvm --output stage1_seen`; `./stage1_seen build …` succeeds.
- Import/bundler
  - Implement `import` resolution; bundle only required modules (e.g., `CompileSeenProgram` and deps).
  - Acceptance: no manual stubs; full Stage‑1 build succeeds.
- Stage‑2/Stage‑3 determinism
  - Build Stage‑2 with Stage‑1, Stage‑3 with Stage‑2; compare hashes.
  - Acceptance: `sha256(stage2_seen) == sha256(stage3_seen)`.
- Tests and docs
  - Add targeted tests: import syntax, triple‑quoted and interpolated strings, string method lowering, bundler integration, simple e2e C build.
  - Keep quickstart and verifier instructions updated.

## Non‑Goals for MVP
- RISC‑V/ISA/vector support, LLVM performance targets, full Kotlin‑feature parity, and complete LSP. These are follow‑ups.

## Risks
- Type/codegen gaps causing C type errors.
- Over/under‑bundling causing duplicates or missing symbols.
- Hidden dependencies in `compiler_seen` that require more runtime helpers.

## Current Reality Checklist
- [x] CLI builds and basic flows work
- [x] Stage‑1 C emits and compiles with temporary stubs
- [ ] Stage‑1 compiles without stubs
- [ ] Bundler resolves imports
- [ ] Stage‑2/Stage‑3 deterministic
- [ ] Tests for bootstrap surface
