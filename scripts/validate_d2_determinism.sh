#!/usr/bin/env bash
# D2 Validation: Fixed-point determinism (deterministic profile)
# Builds stage1 -> stage2 -> stage3 -> stage4 and asserts stage3 == stage4
#
# Why stage3 == stage4 (not stage2 == stage3)?
#   stage1 is built by the Rust bootstrap compiler (different implementation),
#   so stage2 (built by stage1) will differ from stage3 (built by stage2).
#   The true fixed-point property is: once the self-hosted compiler compiles
#   itself twice, the output stabilizes (stage3 == stage4).

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

# Default epoch for deterministic builds if caller has not set one.
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-0}"

echo "🎯 D2 Validation: Stage3 == Stage4 fixed-point determinism"
echo "ROOT: $ROOT"
echo "SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH"
echo

echo "🧹 Cleaning previous stage artifacts"
rm -f stage1.out stage2.out stage3.out stage4.out stage1.o stage2.o stage3.o stage4.o stage1_tmp.out stage1_tmp.o

BOOTSTRAP_SEEN=${BOOTSTRAP_SEEN:-"$ROOT/target-wsl/release/seen_cli"}
USE_SELF_HOSTED=0
if [ -x "$BOOTSTRAP_SEEN" ]; then
    echo "🟢 Using bootstrap compiler: $BOOTSTRAP_SEEN"
    USE_SELF_HOSTED=1
else
    echo "🟡 No bootstrap compiler found at $BOOTSTRAP_SEEN"
    echo "🦀 Falling back to Rust seen_cli (cargo build)"
fi

if [ $USE_SELF_HOSTED -eq 0 ]; then
    CARGO_TARGET_DIR=target-wsl cargo build -p seen_cli --release
    BOOTSTRAP_SEEN="$ROOT/target-wsl/release/seen_cli"
fi

echo "🚀 Building stage1 via $BOOTSTRAP_SEEN"
SEEN_ENABLE_MANIFEST_MODULES=1 SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH \
    "$BOOTSTRAP_SEEN" build compiler_seen/src/main_compiler.seen \
    --backend llvm -o stage1.out

echo "🔁 Building stage2 via stage1"
SEEN_ENABLE_MANIFEST_MODULES=1 SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH ./stage1.out compile compiler_seen/src/main_compiler.seen stage2.out

echo "🔁 Building stage3 via stage2"
SEEN_ENABLE_MANIFEST_MODULES=1 SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH ./stage2.out compile compiler_seen/src/main_compiler.seen stage3.out

echo "🔁 Building stage4 via stage3"
SEEN_ENABLE_MANIFEST_MODULES=1 SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH ./stage3.out compile compiler_seen/src/main_compiler.seen stage4.out

echo
sha256sum stage1.out stage2.out stage3.out stage4.out

echo
if cmp -s stage3.out stage4.out; then
    echo "✅ D2 PASS: stage3.out == stage4.out (fixed-point reached)"
else
    echo "❌ D2 FAIL: stage3.out and stage4.out differ"
    exit 1
fi
