# Quickstart

This guide gets you installing Seen, verifying prerequisites, and running a basic build/check. It assumes Linux/macOS with a C toolchain.

## Prerequisites
- Rust toolchain: `rustc`, `cargo`
- C compiler: `gcc` or `clang`
- Make (optional)

Check installed tools:
- `command -v cargo && cargo --version`
- `command -v rustc && rustc --version`
- `command -v gcc || command -v clang`

Install (Debian/Ubuntu):
- Rust: `curl https://sh.rustup.rs -sSf | sh` (then `source $HOME/.cargo/env`)
- GCC: `sudo apt-get update && sudo apt-get install -y build-essential`

Install (macOS):
- Xcode CLT: `xcode-select --install`
- Rust: `curl https://sh.rustup.rs -sSf | sh`

## Build the CLI
- From repo root: `cargo build -p seen_cli --release`
- Optionally install system‑wide: `sudo ln -sf ./target/release/seen /usr/local/bin/seen`
  - If your build produced `seen_cli`, link that: `sudo ln -sf ./target/release/seen_cli /usr/local/bin/seen`
- Verify: `seen --help`

To enable the LLVM backend (native binaries), build with the LLVM feature:
- `cargo clean && cargo build -p seen_cli --release --features seen_ir/llvm`
  - Requires LLVM toolchain installed (clang/llvm dev libraries)

## Verify the toolchain
- Syntax check the bootstrap entry: `seen check compiler_seen/src/main.seen`
- Emit textual IR (default backend): `seen build compiler_seen/src/main.seen --output stage1.ir`
- Build native with LLVM (requires feature build):
  - `seen build compiler_seen/src/main.seen --backend llvm --output stage1_seen`
- Run directly via interpreter (pure Rust):
  - `seen run compiler_seen/src/main.seen`

Note: The legacy C backend is removed. Use IR (text) or LLVM (native) backends.

## Troubleshooting
- If `seen` is not found, run it from the build folder: `./target/release/seen --help`
- If C compilation fails, ensure prerequisites are installed and follow docs/SELF_HOSTING_PLAN.md to complete bootstrap helpers and bundling.
