#!/usr/bin/env bash
# D2 Validation: Stage2/Stage3 determinism (deterministic profile)
# Builds stage1 -> stage2 -> stage3 with deterministic settings and asserts byte equality

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

# Default epoch for deterministic builds if caller has not set one.
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-0}"

echo "🎯 D2 Validation: Stage2 == Stage3 determinism"
echo "ROOT: $ROOT"
echo "SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH"
echo

echo "🧹 Cleaning previous stage artifacts"
rm -f stage1.out stage2.out stage3.out stage1.o stage2.o stage3.o stage1_tmp.out stage1_tmp.o

echo "🦀 Building stage0 (seen_cli release w/ LLVM)"
cargo build -p seen_cli --release --features llvm

echo "🚀 Building stage1 via seen_cli"
SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH ./target/release/seen_cli build compiler_seen/src/main_compiler.seen \
    --backend llvm --profile deterministic --simd off -o stage1.out

echo "🔁 Building stage2 via stage1"
SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH ./stage1.out compile compiler_seen/src/main_compiler.seen stage2.out

echo "🔁 Building stage3 via stage2"
SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH ./stage2.out compile compiler_seen/src/main_compiler.seen stage3.out

echo
sha256sum stage1.out stage2.out stage3.out

echo
if cmp -s stage2.out stage3.out; then
    echo "✅ D2 PASS: stage2.out == stage3.out"
else
    echo "❌ D2 FAIL: stage2.out and stage3.out differ"
    exit 1
fi
