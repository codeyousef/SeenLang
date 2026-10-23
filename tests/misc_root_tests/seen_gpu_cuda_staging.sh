#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-cuda-staging
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-cuda-staging.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-cuda-staging
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fixture="$ROOT_DIR/tests/production_gpu_runtime/cuda_staging"
run_compiler check "$fixture/src/main.seen" > "$TMP_DIR/frontend.log" 2>&1
for purpose in move share; do
    expected=E_OWNERSHIP_USE_AFTER_MOVE
    if [ "$purpose" = share ]; then expected=E_TYPE_BOUND; fi
    if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/cuda_staging_${purpose}_rejected.seen" > "$TMP_DIR/$purpose-rejected.log" 2>&1; then
        echo 'FAIL: CUDA staging escaped move or creator confinement' >&2; exit 1
    fi
    rg -Fq "$expected" "$TMP_DIR/$purpose-rejected.log"
done
if [ "${SEEN_CUDA_STAGING_HARDWARE:-0}" != 1 ]; then
    echo 'PASS: CUDA staging frontend; hardware not requested'
    exit 0
fi
timeout --foreground --kill-after=5s 30s nvidia-smi --query-gpu=name,uuid --format=csv,noheader > "$TMP_DIR/hardware.log"
rg -Fq 'NVIDIA GeForce RTX 4090' "$TMP_DIR/hardware.log"
timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror -fPIC -shared \
    -I /opt/cuda/include "$ROOT_DIR/tests/production_gpu_runtime/cuda_staging_fault.c" -ldl -o "$TMP_DIR/staging-fault.so"
timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror -fPIC -shared -fsanitize=address \
    -I /opt/cuda/include "$ROOT_DIR/tests/production_gpu_runtime/cuda_staging_fault.c" -ldl -o "$TMP_DIR/staging-fault-address.so"
FIXTURE_ASAN_PRELOAD_LIBRARY="$TMP_DIR/staging-fault-address.so"
for profile in fast release undefined address; do
    (cd "$fixture"; compile_fixture "$profile" staging src/main.seen)
    for mode in reference host-second event-second copy-error record-error query-error close-host close-stream copy-second-error; do
        SEEN_CUDA_STAGING_FAULT="$mode" LD_PRELOAD="$TMP_DIR/staging-fault.so" run_fixture "$profile" staging "$mode"
        cp -- "$TMP_DIR/staging-$profile.log" "$TMP_DIR/staging-$profile-$mode.log"
        cp -- "$TMP_DIR/staging-$profile.run.status" "$TMP_DIR/staging-$profile-$mode.status"
        rg -Fq 'PASS: CUDA pinned staging' "$TMP_DIR/staging-$profile-$mode.log"
        rg -Fq 'drained=1 waits=0' "$TMP_DIR/staging-$profile-$mode.log"
    done
    rg -Fq 'CUDA_STAGING_TRACE host=3 device=3 stream=1 event=3 copies=192 record=192' "$TMP_DIR/staging-$profile-reference.log"
done
timeout --foreground --kill-after=5s 180s /opt/cuda/bin/compute-sanitizer --tool memcheck \
    --leak-check full --error-exitcode 99 "$TMP_DIR/staging-fast" reference > "$TMP_DIR/compute-sanitizer.log" 2>&1
rg -Fq 'ERROR SUMMARY: 0 errors' "$TMP_DIR/compute-sanitizer.log"
echo 'PASS: real RTX 4090 pinned staging byte parity ownership and device memcheck'
