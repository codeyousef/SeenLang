#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: CUDA certification requires the verified serial hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

BUILD_DIR="${SEEN_ARTIFACT_ROOT:?}/cuda-foundation"
cmake -S "$ROOT_DIR/seen_runtime/cuda" -B "$BUILD_DIR/build" \
    -DSEEN_ENABLE_CUDA=ON -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DCMAKE_CUDA_ARCHITECTURES=89
cmake --build "$BUILD_DIR/build" --parallel 1

NVCC="${SEEN_NVCC:-/opt/cuda/bin/nvcc}"
"$NVCC" -std=c++17 -arch=sm_89 -O2 --threads 1 \
    -I "$ROOT_DIR/seen_runtime/cuda/include" \
    "$ROOT_DIR/tests/misc_root_tests/qwen_cuda_foundation.cu" \
    -L "$BUILD_DIR/build" -Xlinker=-rpath -Xlinker="$BUILD_DIR/build" \
    -lseen_cuda \
    -o "$BUILD_DIR/qwen_cuda_foundation"
if [ "${SEEN_QWEN_REQUIRE_CUDA:-0}" = 1 ]; then
    if [ "${SEEN_QWEN_COMPUTE_SANITIZER:-0}" = 1 ]; then
        SEEN_QWEN_SKIP_OOM_PROBE=1 /opt/cuda/bin/compute-sanitizer \
            --tool memcheck --leak-check full --error-exitcode 99 \
            "$BUILD_DIR/qwen_cuda_foundation"
    else
        "$BUILD_DIR/qwen_cuda_foundation"
    fi
else
    echo "PASS: CUDA foundation compile-only (set SEEN_QWEN_REQUIRE_CUDA=1 for hardware certification)"
fi
