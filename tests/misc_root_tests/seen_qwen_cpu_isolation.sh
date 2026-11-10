#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-qwen-cpu-isolation
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"

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

for binary in "$COMPILER" "$ROOT_DIR/compiler_seen/target/seen" \
    "$ROOT_DIR/compiler_seen/target/seen-dev"; do
    if [ "$binary" != "$COMPILER" ] && [ ! -e "$binary" ]; then continue; fi
    [ -f "$binary" ] && [ ! -L "$binary" ] && [ -x "$binary" ] || {
        echo "FAIL: CPU compiler is unavailable or unsafe: $binary" >&2
        exit 126
    }
    if ! dynamic_tags=$(timeout --kill-after=2s 10s readelf -d "$binary" 2>&1); then
        echo "FAIL: cannot inspect CPU compiler ELF dependencies: $binary" >&2
        exit 1
    fi
    dependencies="$dynamic_tags"
    if grep -Fq '(NEEDED)' <<< "$dynamic_tags"; then
        if ! dependencies=$(timeout --kill-after=2s 10s ldd "$binary" 2>&1); then
            echo "FAIL: cannot resolve CPU compiler dependencies: $binary" >&2
            exit 1
        fi
    fi
    if grep -Eiq 'cuda|cublas|nvidia' <<< "$dependencies"; then
        echo "FAIL: CPU compiler links an accelerator library: $binary" >&2
        exit 1
    fi
done

echo "PASS: CPU-only configuration performs no CUDA discovery or linkage"
