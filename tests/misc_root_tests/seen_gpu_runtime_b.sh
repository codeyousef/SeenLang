#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-runtime-b.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-runtime-b
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fixtures=(resource_foundations resource_geometry resource_ranges resource_allocator resource_adverse resource_stress resource_cleanup_budget)
for profile in fast release undefined address; do
    for name in "${fixtures[@]}"; do
        compile_case "$profile" "$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
        rg -Fq 'PASS: GPU-006B' "$TMP_DIR/$name-$profile.log"
    done
done
readelf -d "$TMP_DIR/resource_allocator-fast" > "$TMP_DIR/explicit-provider-libraries.txt"
if rg -i 'NEEDED.*(cuda|cudart|vulkan|amdhip|metal)' "$TMP_DIR/explicit-provider-libraries.txt"; then
    echo 'FAIL: allocator with an explicit test provider linked an accelerator SDK' >&2
    exit 1
fi
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
native_fixtures=(vulkan_resource vulkan_properties)
fault_modes=(buffer-status buffer-empty requirements-size requirements-align-zero requirements-align-three requirements-types memory-status memory-empty bind-status memory-kind memory-heap binding-alignment-zero)
fault_codes=(gpu.out_of_memory gpu.invalid gpu.limit gpu.limit gpu.limit gpu.unsupported_capability gpu.out_of_memory gpu.invalid gpu.device_lost gpu.unsupported_capability gpu.out_of_memory gpu.invalid)
run_native_faults() {
    local profile=$1 name=$2 index
    for index in "${!fault_modes[@]}"; do
        seen_run_vulkan_fault "$profile" "$name" "${fault_modes[index]}" "${fault_codes[index]}"
    done
}
for profile in fast release undefined address; do
    for name in "${native_fixtures[@]}"; do
        compile_fixture "$profile" "$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
        seen_run_vulkan_reference "$profile" "$name"
    done
    compile_fixture "$profile" vulkan_resource_fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_resource_fault_contract.seen"
    run_native_faults "$profile" vulkan_resource_fault
done
FIXTURE_COMPILE_FLAGS=(--coverage)
coverage=()
for name in "${fixtures[@]}" "${native_fixtures[@]}" vulkan_properties_fuzz; do
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    source="$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
    if [ "$name" = vulkan_properties_fuzz ]; then source="$ROOT_DIR/tests/production_gpu_runtime/$name.seen"; fi
    compile_case fast "coverage-$name" "$source"
    coverage+=(--fixture "$TMP_DIR/coverage-$name-fast" "$TMP_DIR/$name.profraw")
done
compile_fixture fast coverage-vulkan_resource_fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_resource_fault_contract.seen"
run_native_faults fast coverage-vulkan_resource_fault
llvm-profdata merge -sparse "$TMP_DIR"/fault-coverage-vulkan_resource_fault-*.profraw -o "$TMP_DIR/fault.profdata"
coverage+=(--fixture "$TMP_DIR/coverage-vulkan_resource_fault-fast" "$TMP_DIR/fault.profdata")
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/resources" "${coverage[@]}" \
    --source seen_std/src/compute/memory_kind.seen \
    --source seen_std/src/compute/resource_stats.seen \
    --source seen_std/src/compute/resource_geometry.seen \
    --source seen_std/src/compute/resource_ranges.seen \
    --source seen_std/src/compute/resource_provider.seen \
    --source seen_std/src/compute/resource_arena_core.seen \
    --source seen_std/src/compute/allocator.seen \
    --source seen_std/src/compute/buffer.seen \
    --source seen_std/src/compute/vulkan_buffer_abi.seen \
    --source seen_std/src/compute/vulkan_memory_info.seen \
    --source seen_std/src/compute/vulkan_memory.seen
python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" "$TMP_DIR/resources-coverage.json" \
    --source seen_std/src/compute/vulkan_memory_info.seen --min-lines 90 --min-branches 90 \
    > "$TMP_DIR/memory-parser-coverage-summary.json"
FIXTURE_COMPILE_FLAGS=()
unset LLVM_PROFILE_FILE
compile_case fast geometry-fuzz "$ROOT_DIR/tests/production_gpu_runtime/resource_geometry_fuzz.seen"
rg -Fq 'PASS: GPU-006B geometry fuzz 60s seed=1101' "$TMP_DIR/geometry-fuzz-fast.log"
for name in buffer allocator; do
    for purpose in move share; do
        expected=E_OWNERSHIP_USE_AFTER_MOVE
        if [ "$purpose" = share ]; then expected=E_TYPE_BOUND; fi
        if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/${name}_${purpose}_rejected.seen" > "$TMP_DIR/$name-$purpose.log" 2>&1; then
            echo 'FAIL: GPU resource owner escaped move or creator confinement' >&2; exit 1
        fi
        rg -Fq "$expected" "$TMP_DIR/$name-$purpose.log"
    done
done
echo 'PASS: typed pooled buffers native Vulkan software reference ownership and accounting; hardware certification unverified'
