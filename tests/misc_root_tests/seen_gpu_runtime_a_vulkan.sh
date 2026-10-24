#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-a-vulkan
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-runtime-a-vulkan.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-runtime-a-vulkan
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
# Native execution deliberately selects the software reference driver. This is
# not hardware certification and production callers must opt into software use.
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
fault_modes=(version-status version-old instance-status instance-empty devices-count-status devices-limit devices-fill-status devices-overflow queue-limit queue-overflow queue-none context-status context-empty)
fault_codes=(gpu.unavailable gpu.unsupported_capability gpu.out_of_memory gpu.invalid gpu.device_lost gpu.limit gpu.invalid gpu.invalid gpu.limit gpu.invalid gpu.unsupported_capability gpu.out_of_memory gpu.invalid)
run_vendor_faults() {
    local profile=$1 name=$2 index
    for index in "${!fault_modes[@]}"; do
        seen_run_vulkan_fault "$profile" "$name" "${fault_modes[index]}" "${fault_codes[index]}"
    done
}
native_fixtures=(vulkan_abi vulkan_instance vulkan_properties vulkan_provider)
for profile in fast release undefined address; do
    for name in "${native_fixtures[@]}"; do
        compile_case "$profile" "$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
        rg -Fq 'PASS:' "$TMP_DIR/$name-$profile.log"
    done
    compile_fixture "$profile" vulkan_fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_fault_contract.seen"
    run_vendor_faults "$profile" vulkan_fault
done
FIXTURE_COMPILE_FLAGS=(--coverage)
compile_fixture fast coverage-vulkan_fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_fault_contract.seen"
run_vendor_faults fast coverage-vulkan_fault
seen_source_coverage_tool llvm-profdata merge -sparse --num-threads=1 \
    "$TMP_DIR"/fault-coverage-vulkan_fault-*.profraw -o "$TMP_DIR/fault.profdata"
native_coverage=(--fixture "$TMP_DIR/coverage-vulkan_fault-fast" "$TMP_DIR/fault.profdata")
for name in "${native_fixtures[@]}" vulkan_properties_fuzz; do
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    source="$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
    if [ "$name" = vulkan_properties_fuzz ]; then source="$ROOT_DIR/tests/production_gpu_runtime/$name.seen"; fi
    compile_case fast "coverage-$name" "$source"
    native_coverage+=(--fixture "$TMP_DIR/coverage-$name-fast" "$TMP_DIR/$name.profraw")
done
seen_verify_source_coverage "$TMP_DIR/vulkan" "${native_coverage[@]}" \
    --source seen_std/src/compute/vulkan_abi.seen \
    --source seen_std/src/compute/vulkan_error.seen \
    --source seen_std/src/compute/vulkan_instance.seen \
    --source seen_std/src/compute/vulkan_properties.seen \
    --source seen_std/src/compute/vulkan_memory_info.seen \
    --source seen_std/src/compute/vulkan_provider.seen \
    --source seen_std/src/compute/vulkan_queue.seen
python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" "$TMP_DIR/vulkan-coverage.json" \
    --source seen_std/src/compute/vulkan_properties.seen --source seen_std/src/compute/vulkan_memory_info.seen --min-lines 90 --min-branches 90 \
    > "$TMP_DIR/vulkan-parser-coverage-summary.json"
echo 'PASS: explicit Vulkan software reference discovery and parser source coverage; hardware execution unverified'
