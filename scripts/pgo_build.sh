#!/bin/bash
# PGO (Profile-Guided Optimization) build script for Seen programs.
# Usage: ./scripts/pgo_build.sh <source.seen> <output> [training_args...]
#
# This script performs a 4-phase PGO build:
#   Phase 1: Compile with instrumentation (--pgo-generate)
#   Phase 2: Run the instrumented binary to collect profiles
#   Phase 3: Merge raw profiles into usable form
#   Phase 4: Recompile with profile data (--pgo-use)

set -e

if [ $# -lt 2 ]; then
    echo "Usage: $0 <source.seen> <output> [training_args...]"
    echo "Example: $0 benchmarks/matrix_multiply.seen matrix_multiply"
    exit 1
fi

SOURCE="$1"
OUTPUT="$2"
shift 2
TRAINING_ARGS="$@"

COMPILER="./compiler_seen/target/seen"
if [ ! -f "$COMPILER" ]; then
    COMPILER="./bootstrap/stage1_frozen"
fi

PROFRAW="/tmp/seen_pgo_${OUTPUT##*/}.profraw"
PROFDATA="/tmp/seen_pgo_${OUTPUT##*/}.profdata"

echo "=== PGO Build: $SOURCE -> $OUTPUT ==="

# Phase 1: Instrumented build
echo "[Phase 1/4] Compiling with instrumentation..."
$COMPILER build "$SOURCE" "${OUTPUT}_instrumented" --pgo-generate
echo "  Instrumented binary: ${OUTPUT}_instrumented"

# Phase 2: Collect profiles
echo "[Phase 2/4] Running instrumented binary to collect profile..."
LLVM_PROFILE_FILE="$PROFRAW" ./"${OUTPUT}_instrumented" $TRAINING_ARGS
echo "  Raw profile: $PROFRAW"

# Phase 3: Merge profiles
echo "[Phase 3/4] Merging profile data..."
llvm-profdata merge -sparse "$PROFRAW" -o "$PROFDATA"
echo "  Merged profile: $PROFDATA"

# Phase 4: Optimized build with profile
echo "[Phase 4/4] Compiling with profile-guided optimization..."
$COMPILER build "$SOURCE" "$OUTPUT" --pgo-use="$PROFDATA"
echo "  PGO-optimized binary: $OUTPUT"

# Cleanup
rm -f "${OUTPUT}_instrumented" "$PROFRAW"

echo "=== PGO Build Complete ==="
echo "Run: ./$OUTPUT"
