# Quickstart

This guide gets you installing Seen, verifying prerequisites, and running a basic build/check. It assumes Linux/macOS with a C toolchain.

## Prerequisites
- Rust toolchain: `rustc`, `cargo`
- LLVM 15 toolchain for the LLVM backend: `llvm-15-dev`, `clang-15`

Check installed tools:
- `command -v cargo && cargo --version`
- `command -v rustc && rustc --version`
- `llvm-config-15 --version` (should print `15.0.7`)

Install (Debian/Ubuntu):
- Rust: `curl https://sh.rustup.rs -sSf | sh` (then `source $HOME/.cargo/env`)
- LLVM: `sudo apt-get update && sudo apt-get install -y llvm-15-dev clang-15`
  - Set env: `export LLVM_SYS_150_PREFIX=/usr/lib/llvm-15` and `export LLVM_CONFIG_PATH=/usr/bin/llvm-config-15`

Install (macOS):
- Xcode CLT: `xcode-select --install`
- Rust: `curl https://sh.rustup.rs -sSf | sh`
- LLVM via Homebrew: `brew install llvm@15` then export:
  - `export LLVM_SYS_150_PREFIX=$(brew --prefix llvm@15)`
  - `export LLVM_CONFIG_PATH=$(brew --prefix llvm@15)/bin/llvm-config`

## Build the CLI
- From repo root (pure Rust/IR path): `cargo build -p seen_cli --release`
- Enable LLVM backend (native binaries): `cargo build -p seen_cli --release --features llvm`
- Optionally install a symlink: `sudo ln -sf ./target/release/seen_cli /usr/local/bin/seen`
- Verify: `seen --help`

## Verify the toolchain
- Syntax check the bootstrap entry: `seen check compiler_seen/src/main.seen`
- Emit textual IR (default backend): `seen build compiler_seen/src/main.seen --output stage1.ir`
- Build native with LLVM (requires feature build): `seen build compiler_seen/src/main.seen --backend llvm --output stage1_seen`
- Run directly via interpreter (pure Rust):
  - `seen run compiler_seen/src/main.seen`

Note: The legacy C backend is removed. Use IR (text) or LLVM (native) backends.

## Run the Verifier
- Determinism of the pipeline (IR text): `seen determinism compiler_seen/src/main.seen -O2`
- Full self‑hosting (LLVM backend):
  - Ensure LLVM 15 env is exported (see above).
  - Run `scripts/self_host_llvm.sh` to build Stage‑1/2/3 and print hashes.

## Troubleshooting
- If `seen` is not found, run it from the build folder: `./target/release/seen --help`
- If C compilation fails, ensure prerequisites are installed and follow docs/SELF_HOSTING_PLAN.md to complete bootstrap helpers and bundling.
