#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: mapped close ownership requires verified serial containment" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

BUILD_DIR="${SEEN_ARTIFACT_ROOT:?}/mapped-close-ownership"
mkdir -p "$BUILD_DIR"
clang -std=c11 -O2 -DSEEN_RUNTIME_TESTING -I "$ROOT_DIR/seen_runtime" \
    "$ROOT_DIR/tests/misc_root_tests/mapped_close_ownership.c" \
    "$ROOT_DIR/seen_runtime/seen_runtime.c" -pthread -ldl -lm \
    -o "$BUILD_DIR/test"
"$BUILD_DIR/test" "$BUILD_DIR/input.bin"

grep -Fq 'Result<Bool, MappedWindowCloseFailure>' \
    "$ROOT_DIR/seen_std/src/memory/mapped_file.seen"
grep -Fq 'Result<Bool, MappedFileCloseFailure>' \
    "$ROOT_DIR/seen_std/src/memory/mapped_file.seen"
echo "PASS: mapped close failures preserve one retryable owner"
