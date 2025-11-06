# Quickstart

This guide gets you installing Seen, verifying prerequisites, and running a basic build/check. It assumes Linux/macOS with a C toolchain.

## Prerequisites
- Rust toolchain: `rustc`, `cargo`
- LLVM 15 toolchain for the LLVM backend: `llvm-15-dev`, `clang-15`
- Optional target tooling:
    - WebAssembly: ensure `wasm-ld` from LLVM 15 is on `PATH`.
    - Android: install Android NDK (r25 or newer) and export `ANDROID_NDK_HOME`; override the default API level via
      `ANDROID_API_LEVEL` if you need a different minimum SDK.

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
- Cross-compile with LLVM (automatic toolchain selection):
    - Linux → AArch64 (clang/LLD):
      `seen build compiler_seen/src/main.seen --backend llvm --target aarch64-unknown-linux-gnu --output stage1_aarch64`
    - WebAssembly (wasm-ld):
      `seen build compiler_seen/src/main.seen --backend llvm --target wasm32-unknown-unknown --output stage1.wasm`
        - Add `--wasm-loader` to emit companion JS/HTML loaders next to the `.wasm` binary.
    - Android (NDK clang/LLD):
      `seen build compiler_seen/src/main.seen --backend llvm --target aarch64-linux-android --output libstage1_android.so`
        - Requires `ANDROID_NDK_HOME` (r25+) and optional `ANDROID_API_LEVEL`.
    - Override linker/archiver via `SEEN_LLVM_LINKER`, `SEEN_LLVM_ARCHIVER`, `SEEN_LLVM_RANLIB` if your toolchain lives
      in a non-standard location.
- Run directly via interpreter (pure Rust):
  - `seen run compiler_seen/src/main.seen`

Note: The legacy C backend is removed. Use IR (text) or LLVM (native) backends.

## Sample Projects

- Linux CLI starter: `examples/linux/hello_cli`
- WebAssembly starter (use with `--target wasm32-unknown-unknown --wasm-loader`): `examples/web/hello_wasm`
- Android NDK starter (requires `ANDROID_NDK_HOME`): `examples/android/hello_ndk`

## Deterministic Profile

- Pass `--profile deterministic` to any `seen` command to pin timestamps and temp directories for reproducible builds.
- Example: `seen --profile deterministic build compiler_seen/src/main.seen --backend llvm --output stage1_seen`
- The flag exports `SOURCE_DATE_EPOCH=0` and uses `.seen/tmp` for temp files. Reset to the default profile by omitting
  the flag.

## Run the Verifier
- Determinism of the pipeline (IR text): `seen determinism compiler_seen/src/main.seen -O2`
- Full self‑hosting (LLVM backend):
  - Ensure LLVM 15 env is exported (see above).
  - Run `scripts/self_host_llvm.sh` to build Stage‑1/2/3 and print hashes.

## Troubleshooting
- If `seen` is not found, run it from the build folder: `./target/release/seen --help`
- If C compilation fails, ensure prerequisites are installed and follow docs/SELF_HOSTING_PLAN.md to complete bootstrap helpers and bundling.

## Next Steps

- Review `docs/concurrency-patterns.md` for structured concurrency tips covering `jobs.scope`, channel futures, and
  select patterns that the MVP plan requires.
