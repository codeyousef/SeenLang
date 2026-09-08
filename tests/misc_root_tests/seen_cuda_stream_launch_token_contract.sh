#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: CUDA stream-token contract requires the verified serial hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

BUILD_DIR="${SEEN_ARTIFACT_ROOT:?}/cuda-stream-token-header"
mkdir -p "$BUILD_DIR"
cc -std=c11 -Wall -Wextra -Werror \
    -I "$ROOT_DIR/seen_runtime/cuda/include" \
    "$ROOT_DIR/tests/misc_root_tests/seen_cuda_stream_launch_token_header.c" \
    -o "$BUILD_DIR/header-contract"
"$BUILD_DIR/header-contract"

SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
[ -x "$SEEN_BIN" ] || {
    echo "FAIL: current Seen compiler is unavailable: $SEEN_BIN" >&2
    exit 1
}
(
    cd "$ROOT_DIR/seen_std"
    "$SEEN_BIN" check tests/cuda_stream_launch_token_api.seen
)

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" \
    "$ROOT_DIR/docs/architecture/native-boundaries.json" >/dev/null
python3 "$ROOT_DIR/scripts/check_native_inventory.py" --root "$ROOT_DIR" \
    --check "$ROOT_DIR/docs/architecture/native-inventory.json" >/dev/null
grep -Fq 'seen-cuda-stream-launch-token-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json"
grep -Fq 'seen_cuda_stream_borrow_launch_token' \
    "$ROOT_DIR/docs/architecture/native-boundaries.json"
grep -Fq 'seen_cuda_stream_borrow_launch_token' \
    "$ROOT_DIR/seen_std/src/accelerator/cuda/mod.seen"
if sed -n '/seen_cuda_stream_borrow_launch_token(/,/^}/p' \
    "$ROOT_DIR/seen_runtime/cuda/src/seen_cuda.cu" | \
    grep -Eq 'cuda(Device|Stream)Synchronize'; then
    echo "FAIL: stream-token accessor synchronizes CUDA work" >&2
    exit 1
fi
if grep -Eq 'cuda(Device|Stream)Synchronize' \
    "$ROOT_DIR/tests/misc_root_tests/seen_qwen_stream_adapter.cu"; then
    echo "FAIL: model stream adapter synchronizes CUDA work" >&2
    exit 1
fi

echo "PASS: fixed-width CUDA stream launch token and CPU-only header contract"
