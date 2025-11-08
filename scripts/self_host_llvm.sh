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
./stage1_seen build compiler_seen/src/main.seen stage2_seen

echo "[3/4] Stage-3: self-building with Stage-2"
./stage2_seen build compiler_seen/src/main.seen stage3_seen

echo "[4/4] Hash compare"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum stage2_seen stage3_seen || true
else
  shasum -a 256 stage2_seen stage3_seen || true
fi

CHANNEL_SAMPLE="seen_cli/tests/fixtures/channel_select.seen"
if [[ -f "$CHANNEL_SAMPLE" ]]; then
  echo "[extra] Verifying channel_select via Stage-1/2 LLVM runs"
  STAGE1_OUT=$(./stage1_seen run "$CHANNEL_SAMPLE" 2>&1 || true)
  STAGE2_OUT=$(./stage2_seen run "$CHANNEL_SAMPLE" 2>&1 || true)
  if [[ "$STAGE1_OUT" != "$STAGE2_OUT" ]]; then
    echo "Channel select output mismatch between Stage-1 and Stage-2:" >&2
    echo "---- Stage1 ----" >&2
    printf "%s\n" "$STAGE1_OUT" >&2
    echo "---- Stage2 ----" >&2
    printf "%s\n" "$STAGE2_OUT" >&2
    exit 1
  else
    echo "Channel select output matches across Stage-1/Stage-2."
  fi
else
  echo "[extra] Channel select fixture not found at $CHANNEL_SAMPLE; skipping channel determinism check."
fi

echo "Done. Review outputs in repo root."
