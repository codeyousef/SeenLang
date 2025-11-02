#!/usr/bin/env bash
set -euo pipefail

# Self-hosting pipeline using LLVM backend.
# Prereqs: cargo, clang/llvm installed; build with: cargo build -p seen_cli --release --features llvm

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
CLI_BIN="${CARGO_TARGET_DIR:-$HOME/.cargo/target-shared}/release/seen_cli"

if [[ ! -x "$CLI_BIN" ]]; then
  echo "seen_cli not found at $CLI_BIN" >&2
  echo "Build it with: cargo build -p seen_cli --release --features llvm" >&2
  exit 1
fi

cd "$ROOT_DIR"

echo "[1/4] Stage-1: building native compiler from Seen sources"
"$CLI_BIN" build compiler_seen/src/main.seen --backend llvm --output stage1_seen

echo "[2/4] Stage-2: self-building with Stage-1"
./stage1_seen build compiler_seen/src/main.seen --backend llvm --output stage2_seen || true

echo "[3/4] Stage-3: self-building with Stage-2"
./stage2_seen build compiler_seen/src/main.seen --backend llvm --output stage3_seen || true

echo "[4/4] Hash compare"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum stage2_seen stage3_seen || true
else
  shasum -a 256 stage2_seen stage3_seen || true
fi

echo "Done. Review outputs in repo root."
