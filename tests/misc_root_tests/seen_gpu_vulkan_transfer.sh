#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-vulkan-transfer
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-vulkan-transfer.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-vulkan-transfer
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
export SEEN_VULKAN_FIXTURE_COPY_TRACE=1
faults=(copy-pool-status copy-pool-empty copy-semaphore-status copy-semaphore-empty copy-commands-status
    copy-command-empty copy-command-duplicate copy-reset-status copy-begin-status copy-end-status
    copy-submit-oom copy-submit-lost copy-submit-unknown copy-poll-oom copy-poll-timeout copy-poll-lost
    copy-late-timeout copy-post-submit-timeout timeline-none)
codes=(gpu.out_of_memory gpu.invalid gpu.out_of_memory gpu.invalid gpu.out_of_memory
    gpu.invalid gpu.invalid gpu.out_of_memory gpu.out_of_memory gpu.out_of_memory
    gpu.out_of_memory gpu.device_lost gpu.unavailable gpu.out_of_memory none gpu.device_lost
    gpu.timeout none gpu.unsupported_capability)
run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/vulkan_transfer_contract.seen" > "$TMP_DIR/frontend.log" 2>&1
for profile in fast release undefined address; do
    compile_fixture "$profile" transfer "$ROOT_DIR/tests/production_gpu_runtime/vulkan_transfer_contract.seen"
    seen_run_vulkan_reference "$profile" transfer
    rg -Fq 'VULKAN_COPY_TRACE pool=1/1 semaphore=1/1 alloc=1 commands=2 copy=102 submit=102 buffers=3 memory=3' "$TMP_DIR/transfer-$profile.log"
    compile_fixture "$profile" transfer-adverse "$ROOT_DIR/tests/production_gpu_runtime/vulkan_transfer_adverse_contract.seen"
    seen_run_vulkan_reference "$profile" transfer-adverse
    compile_fixture "$profile" transfer-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_transfer_fault_contract.seen"
    for index in "${!faults[@]}"; do
        seen_run_vulkan_fault "$profile" transfer-fault "${faults[index]}" "${codes[index]}"
    done
done
FIXTURE_COMPILE_FLAGS=(--coverage); coverage=()
for name in transfer transfer-adverse; do
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    source_name=vulkan_transfer_contract.seen
    if [ "$name" = transfer-adverse ]; then source_name=vulkan_transfer_adverse_contract.seen; fi
    compile_fixture fast "coverage-$name" "$ROOT_DIR/tests/production_gpu_runtime/$source_name"
    seen_run_vulkan_reference fast "coverage-$name"
    coverage+=(--fixture "$TMP_DIR/coverage-$name-fast" "$TMP_DIR/$name.profraw")
done
compile_fixture fast coverage-transfer-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_transfer_fault_contract.seen"
for index in "${!faults[@]}"; do
    seen_run_vulkan_fault fast coverage-transfer-fault "${faults[index]}" "${codes[index]}"
done
llvm-profdata merge -sparse "$TMP_DIR"/fault-coverage-transfer-fault-*.profraw -o "$TMP_DIR/fault.profdata"
coverage+=(--fixture "$TMP_DIR/coverage-transfer-fault-fast" "$TMP_DIR/fault.profdata")
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/copies" "${coverage[@]}" \
    --source seen_std/src/compute/vulkan_transfer.seen \
    --source seen_std/src/compute/vulkan_transfer_abi.seen
FIXTURE_COMPILE_FLAGS=(); unset LLVM_PROFILE_FILE
for purpose in move share; do
    expected=E_OWNERSHIP_USE_AFTER_MOVE
    if [ "$purpose" = share ]; then expected=E_TYPE_BOUND; fi
    if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/vulkan_transfer_${purpose}_rejected.seen" > "$TMP_DIR/$purpose-rejected.log" 2>&1; then
        echo 'FAIL: native copy owner escaped move or creator confinement' >&2; exit 1
    fi
    rg -Fq "$expected" "$TMP_DIR/$purpose-rejected.log"
done
echo 'PASS: bounded native Vulkan copy parity failure retry timeline completion ownership and no per-copy native resource creation'
