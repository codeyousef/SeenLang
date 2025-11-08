#!/usr/bin/env bash
set -euo pipefail

# Runs determinism checks for non-LLVM backends so nightly automation can
# exercise MLIR + Cranelift text emission without waiting on LLVM.

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
CLI_BIN="${CARGO_TARGET_DIR:-$HOME/.cargo/target-shared}/release/seen_cli"

if [[ ! -x "$CLI_BIN" ]]; then
  echo "seen_cli not found at $CLI_BIN; building release binary..." >&2
  (cd "$ROOT_DIR" && cargo build -p seen_cli --release) >&2
fi

TARGET="${1:-compiler_seen/src/main.seen}"
OPT_LEVEL="${OPT_LEVEL:-1}"

if [[ "$TARGET" == compiler_seen/src/* ]]; then
  export SEEN_ENABLE_MANIFEST_MODULES=1
fi

cd "$ROOT_DIR"

for backend in mlir clif; do
  echo "[nightly] checking determinism for $backend (O${OPT_LEVEL}) on $TARGET"
  "$CLI_BIN" determinism "$TARGET" --backend "$backend" -O"$OPT_LEVEL"
done

echo "[nightly] non-LLVM backends emitted deterministic artifacts."
