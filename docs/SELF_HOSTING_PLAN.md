# Self‑Hosting Bootstrap Plan

## Goals
- Build and validate a fully working Seen compiler from the Rust workspace, then re‑bootstrap to a self‑hosted compiler from Seen sources.
- Produce a deterministic Stage 2 = Stage 3 binary match and basic IDE/LSP validation.

## Prerequisites
- Rust toolchain installed (rustc, cargo).
- Toolchain for native builds: clang/llc/opt and gcc/make on PATH.
- Verify: `rustc --version && cargo --version && which clang llc opt gcc`.

## Branch & Build (Rust Base)
- Branch: `git checkout -b compiler-first origin/main` (done).
- Build CLI (release):
  - Local target dir: `CARGO_TARGET_DIR=target cargo build -p seen_cli --release`
  - Or shared target: binary at `~/.cargo/target-shared/release/seen_cli`.
- Install CLI: `sudo cp <path>/seen_cli /usr/local/bin/seen && seen --help`.

## Stage 1 (Seen → Native via LLVM)
- Build the CLI with LLVM enabled:
  - `cargo build -p seen_cli --release --features seen_ir/llvm`
- Produce Stage‑1 binary directly:
  - `./target/release/seen_cli build compiler_seen/src/main.seen --backend llvm --output stage1_seen`
- Sanity check: `./stage1_seen --version`.

Status (implemented):
- Added string runtime helpers and method-call lowering (length/endsWith/substring/+).
- Added bootstrap stubs in emitted C for: `CompileSeenProgram`, `println`, file ops and misc.
- Disabled bundling of the full `main_compiler` during Stage 1 to avoid type/IR gaps.

Next:
- Remove stub reliance by lowering lists/struct returns and providing minimal runtime.

## Stage 2/3 (Self‑Compile Twice)
- Build Stage‑2 with Stage‑1:
  - `./stage1_seen build compiler_seen/src/main.seen --backend llvm --output stage2_seen`
- Build Stage‑3 with Stage‑2:
  - `./stage2_seen build compiler_seen/src/main.seen --backend llvm --output stage3_seen`
- Verify determinism:
  - `sha256sum stage2_seen stage3_seen` (expect identical hashes).

## Verifier (Optional)
- Run Seen verifier with Rust CLI:
  - `seen run compiler_seen/src/bootstrap/verifier.seen`
- Or build a native verifier (LLVM):
  - `seen build compiler_seen/src/bootstrap/verifier.seen --backend llvm --output verifier && ./verifier`

## Install Self‑Hosted Compiler
- Backup bootstrap: `sudo mv /usr/local/bin/seen /usr/local/bin/seen_bootstrap_backup`.
- Install: `sudo cp ./stage3_seen /usr/local/bin/seen && sudo chmod +x /usr/local/bin/seen`.
- Validate: `hash -r && seen --help && seen run examples/basic/hello_world.seen`.

## Acceptance
- Stage2 = Stage3 (hash match).
- CLI build/run/check work on examples.
- LSP loads (optional): `cargo test -p seen_lsp` or editor check.
