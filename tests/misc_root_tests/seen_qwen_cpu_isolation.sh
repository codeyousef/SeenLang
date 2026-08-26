#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: CPU dependency isolation requires the verified serial hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

BUILD_DIR="${SEEN_ARTIFACT_ROOT:?}/cpu-isolation"
mkdir -p "$BUILD_DIR"
if cmake -S "$ROOT_DIR/seen_runtime/cuda" -B "$BUILD_DIR/cuda-disabled" \
    -DSEEN_ENABLE_CUDA=OFF >"$BUILD_DIR/configure.log" 2>&1; then
    echo "FAIL: disabled CUDA subsystem unexpectedly configured" >&2
    exit 1
fi
grep -Fq 'Seen CUDA discovery is gated' "$BUILD_DIR/configure.log"
if grep -RIEq 'CMAKE_CUDA_COMPILER|CUDAToolkit|/opt/cuda/(include|lib|targets)|nvcc' \
    "$BUILD_DIR/cuda-disabled"; then
    echo "FAIL: disabled configuration discovered a CUDA toolchain" >&2
    exit 1
fi

for binary in "$ROOT_DIR/compiler_seen/target/seen" \
    "$ROOT_DIR/compiler_seen/target/seen-dev"; do
    [ -x "$binary" ] || continue
    if ldd "$binary" 2>/dev/null | grep -Eiq 'cuda|cublas|nvidia'; then
        echo "FAIL: CPU compiler links an accelerator library: $binary" >&2
        exit 1
    fi
done

echo "PASS: CPU-only configuration performs no CUDA discovery or linkage"
