#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: Qwen Seen contracts require the verified hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
[ -x "$SEEN_BIN" ] || {
    echo "FAIL: current Seen compiler is unavailable: $SEEN_BIN" >&2
    exit 1
}
COMPILE_DIR="${SEEN_ARTIFACT_ROOT:?}/qwen-seen-contracts"
mkdir -p "$COMPILE_DIR"
compile_and_run() {
    local source=$1
    local output=$2
    "$SEEN_BIN" compile "$source" "$output" --fast --no-cache \
        --jobs 1 --opt-jobs 1
    "$output"
}
compile_and_run "$ROOT_DIR/tests/codegen/test_fixed_width_array_storage.seen" \
    "$COMPILE_DIR/fixed-width-array"
compile_and_run "$ROOT_DIR/seen_std/tests/qwen_prerequisites.seen" \
    "$COMPILE_DIR/stdlib-prerequisites"
compile_and_run "$ROOT_DIR/projects/seen_ml/qwen38/tests/prerequisites.seen" \
    "$COMPILE_DIR/project-prerequisites"
echo "PASS: Qwen fixed-width, hash, JSON, Safetensors, lock, and artifact contracts"
