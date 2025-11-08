# Self‑Hosting Bootstrap Plan

## Goals
- Build and validate a fully working Seen compiler from the Rust workspace, then re‑bootstrap to a self‑hosted compiler from Seen sources.
- Produce a deterministic Stage 2 = Stage 3 binary match and basic IDE/LSP validation.

## Prerequisites
- Rust toolchain installed (rustc, cargo).
- LLVM 15 toolchain for native builds: `llvm-15-dev` and `clang-15` (Linux) or Homebrew `llvm@15` (macOS).
- Export env (Linux):
  - `export LLVM_SYS_150_PREFIX=/usr/lib/llvm-15`
  - `export LLVM_CONFIG_PATH=/usr/bin/llvm-config-15`
- Verify: `rustc --version && cargo --version && llvm-config-15 --version && which clang`.

## Branch & Build (Rust Base)
- Branch: `git checkout -b compiler-first origin/main` (done).
- Build CLI (release):
  - Local target dir: `CARGO_TARGET_DIR=target cargo build -p seen_cli --release`
  - Or shared target: binary at `~/.cargo/target-shared/release/seen_cli`.
- Install CLI: `sudo cp <path>/seen_cli /usr/local/bin/seen && seen --help`.

## Stage 1 (Seen → Native via LLVM)
- Build the CLI with LLVM enabled:
  - `cargo build -p seen_cli --release --features llvm`
- Produce Stage‑1 binary directly:
  - `~/.cargo/target-shared/release/seen_cli build compiler_seen/src/main.seen --backend llvm --output stage1_seen`
- Sanity check: `./stage1_seen --version`.

Status (implemented):
- LLVM backend wired (inkwell 0.6, LLVM 15).
- String runtime helpers and method-call lowering (length/endsWith/substring/+).
- Import bundling in CLI (basic path resolution).

Next:
- Expand array/list lowering; implement `CompileSeenProgram` LLVM path; complete type coverage.

## Stage 2/3 (Self‑Compile Twice)
- Build Stage‑2 with Stage‑1:
    - `./stage1_seen build compiler_seen/src/main.seen stage2_seen`
- Build Stage‑3 with Stage‑2:
    - `./stage2_seen build compiler_seen/src/main.seen stage3_seen`
- Note: the Stage‑1/Stage‑2 Seen binaries already drive the LLVM pipeline internally, so additional CLI flags like
  `--backend` or `--output` are not required (the final path is simply the trailing argument).
- Verify determinism:
  - `sha256sum stage2_seen stage3_seen` (expect identical hashes).

Automation:
- `scripts/self_host_llvm.sh` runs Stage‑1/2/3 and prints hashes.

## Verifier (Optional)
- Determinism (IR text): `seen determinism compiler_seen/src/main.seen -O2`.
- Full pipeline (LLVM): `scripts/self_host_llvm.sh` builds Stage‑1/2/3 and prints hashes.

## Install Self‑Hosted Compiler
- Backup bootstrap: `sudo mv /usr/local/bin/seen /usr/local/bin/seen_bootstrap_backup`.
- Install: `sudo cp ./stage3_seen /usr/local/bin/seen && sudo chmod +x /usr/local/bin/seen`.
- Validate: `hash -r && seen --help && seen run examples/basic/hello_world.seen`.

## Acceptance
- Stage2 = Stage3 (hash match).
- CLI build/run/check work on examples.
- LSP loads (optional): `cargo test -p seen_lsp` or editor check.
