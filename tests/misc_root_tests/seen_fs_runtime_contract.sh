#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: filesystem runtime certification requires the verified serial hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

BUILD_DIR="${SEEN_ARTIFACT_ROOT:?}/filesystem-runtime"
mkdir -p "$BUILD_DIR/work"
clang -std=c11 -O1 -I "$ROOT_DIR/seen_runtime" \
    "$ROOT_DIR/tests/misc_root_tests/seen_fs_runtime_contract.c" \
    "$ROOT_DIR/seen_runtime/seen_runtime.c" \
    -pthread -ldl -lm -o "$BUILD_DIR/test"
"$BUILD_DIR/test" "$BUILD_DIR/work"
