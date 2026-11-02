#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-cuda-peer
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-cuda-peer.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-cuda-peer
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fixture="$ROOT_DIR/tests/production_gpu_runtime/cuda_peer/src/main.seen"
run_compiler check "$fixture" > "$TMP_DIR/frontend.log" 2>&1
for profile in fast release undefined address; do
    compile_fixture "$profile" cpu-import "$ROOT_DIR/tests/production_gpu_runtime/cuda_peer_import_cpu.seen"
    run_fixture "$profile" cpu-import
    readelf -d "$TMP_DIR/cpu-import-$profile" > "$TMP_DIR/cpu-import-$profile.libraries"
    if rg -i 'NEEDED.*(cuda|cudart|cublas|vulkan|amdhip|metal)' "$TMP_DIR/cpu-import-$profile.libraries"; then
        echo 'FAIL: unused CUDA peer imports linked an accelerator SDK' >&2; exit 1
    fi
done
if [ "${SEEN_CUDA_PEER_HARDWARE:-0}" != 1 ]; then
    echo 'PASS: CUDA peer frontend and unused-import CPU execution; native hardware not requested'
    exit 0
fi
timeout --foreground --kill-after=5s 30s nvidia-smi --query-gpu=name,uuid --format=csv,noheader > "$TMP_DIR/hardware.log"
rg -Fq 'NVIDIA GeForce RTX 4090' "$TMP_DIR/hardware.log"
for profile in plain address; do
    flags=(); destination="$TMP_DIR/peer-fault.so"
    if [ "$profile" = address ]; then flags=(-fsanitize=address); destination="$TMP_DIR/peer-fault-address.so"; fi
    timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror -fPIC -shared \
        "${flags[@]}" -I /opt/cuda/include "$ROOT_DIR/tests/production_gpu_runtime/cuda_peer_fault.c" -ldl -o "$destination"
done
FIXTURE_ASAN_PRELOAD_LIBRARY="$TMP_DIR/peer-fault-address.so"
FIXTURE_ASAN_ADDRESS_LAYOUT=cuda
native_status=0
for profile in fast release undefined address; do
    compile_fixture "$profile" peer "$fixture"
    for mode in reference copy-second-error peer-enable-invalid peer-count-negative peer-count-zero peer-count-limit peer-access-invalid peer-access-self peer-query-error peer-query-oom; do
        arguments=("$mode")
        case "$mode" in
            peer-count-negative|peer-access-invalid|peer-access-self) arguments+=(gpu.invalid) ;;
            peer-count-zero) arguments+=(gpu.unsupported_capability) ;;
            peer-count-limit) arguments+=(gpu.limit) ;;
            peer-query-error) arguments+=(gpu.unavailable) ;;
            peer-query-oom) arguments+=(gpu.out_of_memory) ;;
        esac
        status=0
        SEEN_CUDA_STAGING_FAULT="$mode" LD_PRELOAD="$TMP_DIR/peer-fault.so" run_fixture "$profile" peer "${arguments[@]}" || status=$?
        cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/peer-$profile-$mode.log"
        cp -- "$TMP_DIR/peer-$profile.run.status" "$TMP_DIR/peer-$profile-$mode.status"
        if [ "$status" != 0 ]; then
            if [ "$native_status" = 0 ]; then native_status=$status; fi
        elif [ "${mode#peer-}" = "$mode" ] && ! rg -Fq 'drained=1 waits=0' "$TMP_DIR/peer-$profile-$mode.log"; then
            if [ "$native_status" = 0 ]; then native_status=1; fi
        fi
    done
done
[ "$native_status" = 0 ] || exit "$native_status"
echo 'PASS: explicit CUDA native peer capability and retained device transfers; physical route classification is recorded per run'
