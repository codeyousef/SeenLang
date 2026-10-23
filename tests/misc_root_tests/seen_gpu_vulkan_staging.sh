#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-vulkan-staging
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-vulkan-staging.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-vulkan-staging
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
export SEEN_VULKAN_FIXTURE_COPY_TRACE=1 SEEN_VULKAN_FIXTURE_MAP_TRACE=1
faults=(copy-pool-status copy-pool-empty copy-semaphore-status copy-semaphore-empty copy-commands-status
    copy-command-empty copy-command-duplicate timeline-none buffer-status buffer-empty memory-status memory-empty
    bind-status staging-buffer-second map-status map-empty map-address-overflow map-span-overflow
    copy-reset-status copy-begin-status copy-end-status copy-submit-oom copy-submit-lost copy-submit-unknown
    copy-poll-oom copy-poll-timeout copy-poll-lost copy-late-timeout copy-post-submit-timeout)
codes=(gpu.out_of_memory gpu.invalid gpu.out_of_memory gpu.invalid gpu.out_of_memory
    gpu.invalid gpu.invalid gpu.unsupported_capability gpu.out_of_memory gpu.invalid gpu.out_of_memory gpu.invalid
    gpu.device_lost gpu.out_of_memory gpu.unavailable gpu.invalid gpu.invalid gpu.invalid
    gpu.out_of_memory gpu.out_of_memory gpu.out_of_memory gpu.out_of_memory gpu.device_lost gpu.unavailable
    gpu.out_of_memory none gpu.device_lost gpu.timeout none)
for name in staging staging-adverse staging-fault; do
    source_name=vulkan_staging_contract.seen
    if [ "$name" = staging-adverse ]; then source_name=vulkan_staging_adverse_contract.seen; fi
    if [ "$name" = staging-fault ]; then source_name=vulkan_staging_fault_contract.seen; fi
    run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/$source_name" > "$TMP_DIR/$name-frontend.log" 2>&1
done
for profile in fast release undefined address; do
    compile_fixture "$profile" staging "$ROOT_DIR/tests/production_gpu_runtime/vulkan_staging_contract.seen"
    seen_run_vulkan_reference "$profile" staging
    # Two logical staging slots share one native arena block; the device
    # destination owns the other. Neither count grows over the 100 copies.
    rg -Fq 'VULKAN_COPY_TRACE pool=1/1 semaphore=1/1 alloc=1 commands=2 copy=100 submit=100 buffers=2 memory=2' "$TMP_DIR/staging-$profile.log"
    compile_fixture "$profile" staging-adverse "$ROOT_DIR/tests/production_gpu_runtime/vulkan_staging_adverse_contract.seen"
    seen_run_vulkan_reference "$profile" staging-adverse
    compile_fixture "$profile" staging-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_staging_fault_contract.seen"
    for index in "${!faults[@]}"; do
        seen_run_vulkan_fault "$profile" staging-fault "${faults[index]}" "${codes[index]}"
    done
done
FIXTURE_COMPILE_FLAGS=(--coverage); coverage=()
for name in staging staging-adverse; do
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    source_name=vulkan_staging_contract.seen
    if [ "$name" = staging-adverse ]; then source_name=vulkan_staging_adverse_contract.seen; fi
    compile_fixture fast "coverage-$name" "$ROOT_DIR/tests/production_gpu_runtime/$source_name"
    seen_run_vulkan_reference fast "coverage-$name"
    coverage+=(--fixture "$TMP_DIR/coverage-$name-fast" "$TMP_DIR/$name.profraw")
done
compile_fixture fast coverage-staging-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_staging_fault_contract.seen"
for index in "${!faults[@]}"; do
    seen_run_vulkan_fault fast coverage-staging-fault "${faults[index]}" "${codes[index]}"
done
llvm-profdata merge -sparse "$TMP_DIR"/fault-coverage-staging-fault-*.profraw -o "$TMP_DIR/fault.profdata"
coverage+=(--fixture "$TMP_DIR/coverage-staging-fault-fast" "$TMP_DIR/fault.profdata")
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/staging" "${coverage[@]}" --source seen_std/src/compute/vulkan_staging.seen
FIXTURE_COMPILE_FLAGS=(); unset LLVM_PROFILE_FILE
for purpose in move share; do
    expected=E_OWNERSHIP_USE_AFTER_MOVE
    if [ "$purpose" = share ]; then expected=E_TYPE_BOUND; fi
    if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/vulkan_staging_${purpose}_rejected.seen" > "$TMP_DIR/$purpose-rejected.log" 2>&1; then
        echo 'FAIL: staging owner escaped move or creator confinement' >&2; exit 1
    fi
    rg -Fq "$expected" "$TMP_DIR/$purpose-rejected.log"
done
echo 'PASS: owned Vulkan staging bulk bytes bounds faults completion consumer acknowledgement and cold cleanup'
