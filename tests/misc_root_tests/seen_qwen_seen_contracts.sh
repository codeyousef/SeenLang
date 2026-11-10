#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
SEEN_BIN=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-qwen-seen-contracts
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$SEEN_BIN" -- bash "$0" "$@"
fi
SEEN_BIN="${SEEN_CAPPED_REGRESSION_COMPILER:-$SEEN_BIN}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$SEEN_BIN"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
[ -x "$SEEN_BIN" ] || {
    echo "FAIL: current Seen compiler is unavailable: $SEEN_BIN" >&2
    exit 1
}
COMPILE_DIR="${SEEN_ARTIFACT_ROOT:?}/qwen-seen-contracts"
mkdir -p "$COMPILE_DIR"
compile_and_run() {
    local source=$1
    local output=$2
    bash "$ATTESTED_SEEN" "$SEEN_BIN" compile "$source" "$output" --fast --no-cache
    timeout --kill-after=5s 120s "$output"
}
compile_and_run "$ROOT_DIR/tests/codegen/test_fixed_width_array_storage.seen" \
    "$COMPILE_DIR/fixed-width-array"
compile_and_run "$ROOT_DIR/seen_std/tests/qwen_prerequisites.seen" \
    "$COMPILE_DIR/stdlib-prerequisites"
compile_and_run "$ROOT_DIR/projects/seen_ml/qwen38/tests/prerequisites.seen" \
    "$COMPILE_DIR/project-prerequisites"
echo "PASS: Qwen fixed-width, hash, JSON, Safetensors, lock, and artifact contracts"
