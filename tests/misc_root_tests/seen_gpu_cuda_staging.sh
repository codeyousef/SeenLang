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
for profile in fast release; do
    compile_case "$profile" cpu-import "$ROOT_DIR/tests/production_gpu_runtime/cuda_staging_import_cpu.seen"
    readelf -d "$TMP_DIR/cpu-import-$profile" > "$TMP_DIR/cpu-import-$profile.libraries"
    if rg -i 'NEEDED.*(cuda|cudart|cublas|vulkan|amdhip|metal)' "$TMP_DIR/cpu-import-$profile.libraries"; then
        echo 'FAIL: unused CUDA staging import linked an accelerator SDK' >&2; exit 1
    fi
done
if [ "${SEEN_CUDA_STAGING_HARDWARE:-0}" != 1 ]; then
    echo 'PASS: CUDA staging frontend ownership and CPU-only import; hardware not requested'
    exit 0
fi
timeout --foreground --kill-after=5s 30s nvidia-smi --query-gpu=name,uuid --format=csv,noheader > "$TMP_DIR/hardware.log"
rg -Fq 'NVIDIA GeForce RTX 4090' "$TMP_DIR/hardware.log"
timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror -fPIC -shared \
    -I /opt/cuda/include "$ROOT_DIR/tests/production_gpu_runtime/cuda_staging_fault.c" -ldl -o "$TMP_DIR/staging-fault.so"
timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror -fPIC -shared -fsanitize=address \
    -I /opt/cuda/include "$ROOT_DIR/tests/production_gpu_runtime/cuda_staging_fault.c" -ldl -o "$TMP_DIR/staging-fault-address.so"
FIXTURE_ASAN_PRELOAD_LIBRARY="$TMP_DIR/staging-fault-address.so"
staging_modes=(reference host-second event-second copy-error record-error query-error close-host close-stream copy-second-error cross-ring late-timeout)
run_staging_modes() {
    local profile=$1 name=$2 mode status
    for mode in "${staging_modes[@]}"; do
        status=0
        LLVM_PROFILE_FILE="$TMP_DIR/$name-$profile-$mode.profraw" \
            SEEN_CUDA_STAGING_FAULT="$mode" LD_PRELOAD="$TMP_DIR/staging-fault.so" run_fixture "$profile" "$name" "$mode" || status=$?
        cp -- "$TMP_DIR/$name-$profile.log" "$TMP_DIR/$name-$profile-$mode.log" || return
        cp -- "$TMP_DIR/$name-$profile.run.status" "$TMP_DIR/$name-$profile-$mode.status" || return
        [ "$status" = 0 ] || return "$status"
        rg -Fq 'PASS: CUDA pinned staging' "$TMP_DIR/$name-$profile-$mode.log" || return
        rg -Fq 'drained=1 waits=0' "$TMP_DIR/$name-$profile-$mode.log" || return
    done
}
diagnose_cuda_asan_initialization() {
    local status=0
    # Genuine compiler trace and binary sanitizer symbol are independently
    # checked by the same hard-scope runner. No fabricated instrumentation log.
    (set -x; clang -std=c11 -O1 -g -march=x86-64 -fno-omit-frame-pointer -fsanitize=address \
        -I /opt/cuda/include "$ROOT_DIR/tests/production_gpu_runtime/cuda_initialization_control.c" \
        -L /opt/cuda/lib64 -Wl,-rpath,/opt/cuda/lib64 -lcudart -o "$TMP_DIR/cuda-init-control") \
        > "$TMP_DIR/cuda-init-control.compile.log" 2>&1 || return
    timeout --foreground --kill-after=5s 120s bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
        --target-root "$TMP_DIR" --compile-log "$TMP_DIR/cuda-init-control.compile.log" \
        -- "$TMP_DIR/cuda-init-control" > "$TMP_DIR/cuda-init-control.run.log" 2>&1 || status=$?
    printf '%s\n' "$status" > "$TMP_DIR/cuda-init-control.status" || return
    echo "Independent native CUDA/ASan initialization diagnostic status=$status; original staging failure remains authoritative" >&2
    # Diagnostic completion is not acceptance, regardless of this status.
}
FIXTURE_COMPILE_FLAGS=(--coverage)
(cd "$fixture"; compile_fixture fast coverage src/main.seen)
run_staging_modes fast coverage
profiles=()
for mode in "${staging_modes[@]}"; do profiles+=("$TMP_DIR/coverage-fast-$mode.profraw"); done
llvm-profdata merge -sparse "${profiles[@]}" -o "$TMP_DIR/combined.profdata"
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/staging" --fixture "$TMP_DIR/coverage-fast" "$TMP_DIR/combined.profdata" \
    --source seen_std/src/compute/cuda_staging.seen
FIXTURE_COMPILE_FLAGS=()
for profile in fast release undefined address; do
    (cd "$fixture"; compile_fixture "$profile" staging src/main.seen)
    status=0
    run_staging_modes "$profile" staging || status=$?
    if [ "$status" != 0 ]; then
        if [ "$profile" = address ] && rg -Fq 'gpu.cuda.staging.open' "$TMP_DIR/staging-address.log" &&
            rg -Fq 'gpu.out_of_memory' "$TMP_DIR/staging-address.log"; then
            diagnose_cuda_asan_initialization || echo 'Independent native diagnostic could not complete; preserve the original failure' >&2
        fi
        exit "$status"
    fi
    rg -Fq 'CUDA_STAGING_TRACE host=3 device=3 stream=1 event=3 copies=192 record=192' "$TMP_DIR/staging-$profile-reference.log"
    if [ "$profile" = fast ]; then
        timeout --foreground --kill-after=5s 180s /opt/cuda/bin/compute-sanitizer --tool memcheck \
            --leak-check full --error-exitcode 99 "$TMP_DIR/staging-fast" reference > "$TMP_DIR/compute-sanitizer.log" 2>&1
        rg -Fq 'ERROR SUMMARY: 0 errors' "$TMP_DIR/compute-sanitizer.log"
    fi
done
echo 'PASS: real RTX 4090 pinned staging byte parity ownership and device memcheck'
