#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-staging-foundations
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-staging-foundations.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-staging-foundations
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fixtures=(staging_ring staging_readback staging_adverse buffer_region)
for profile in fast release undefined address; do
    for name in "${fixtures[@]}"; do
        compile_case "$profile" "$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
        rg -Fq 'PASS: GPU-006C' "$TMP_DIR/$name-$profile.log"
    done
done
readelf -d "$TMP_DIR/staging_ring-fast" > "$TMP_DIR/metadata-libraries.txt"
if rg -i 'NEEDED.*(cuda|cudart|vulkan|amdhip|metal)' "$TMP_DIR/metadata-libraries.txt"; then
    echo 'FAIL: staging metadata linked an accelerator SDK' >&2; exit 1
fi
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
native_fixtures=(vulkan_timeline vulkan_timeline_adverse)
fault_modes=(timeline-root-tag timeline-root-chain timeline-tag timeline-chain timeline-bool)
run_timeline_faults() {
    local profile=$1 name=$2 mode
    for mode in "${fault_modes[@]}"; do seen_run_vulkan_fault "$profile" "$name" "$mode" gpu.invalid; done
}
for profile in fast release undefined address; do
    for name in "${native_fixtures[@]}"; do
        compile_fixture "$profile" "$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
        seen_run_vulkan_reference "$profile" "$name"
    done
    SEEN_VULKAN_FIXTURE_FAULT=timeline-none LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
        run_fixture "$profile" vulkan_timeline unsupported
    compile_fixture "$profile" timeline_fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_fault_contract.seen"
    run_timeline_faults "$profile" timeline_fault
done
FIXTURE_COMPILE_FLAGS=(--coverage); coverage=()
for name in "${fixtures[@]}" "${native_fixtures[@]}" vulkan_timeline_fuzz; do
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    source="$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
    if [ "$name" = vulkan_timeline_fuzz ]; then source="$ROOT_DIR/tests/production_gpu_runtime/$name.seen"; fi
    compile_case fast "coverage-$name" "$source"
    coverage+=(--fixture "$TMP_DIR/coverage-$name-fast" "$TMP_DIR/$name.profraw")
done
compile_fixture fast coverage-timeline_fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_fault_contract.seen"
run_timeline_faults fast coverage-timeline_fault
llvm-profdata merge -sparse "$TMP_DIR"/fault-coverage-timeline_fault-*.profraw -o "$TMP_DIR/fault.profdata"
coverage+=(--fixture "$TMP_DIR/coverage-timeline_fault-fast" "$TMP_DIR/fault.profdata")
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/foundations" "${coverage[@]}" \
    --source seen_std/src/compute/staging_ring.seen \
    --source seen_std/src/compute/buffer_region.seen \
    --source seen_std/src/compute/vulkan_timeline.seen
python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" "$TMP_DIR/foundations-coverage.json" \
    --source seen_std/src/compute/vulkan_timeline.seen --min-lines 90 --min-branches 90 \
    > "$TMP_DIR/timeline-parser-coverage-summary.json"
unset LLVM_PROFILE_FILE
FIXTURE_COMPILE_FLAGS=()
for name in staging_ring staging_slice buffer_region; do
    for purpose in move share; do
        expected=E_OWNERSHIP_USE_AFTER_MOVE
        if [ "$purpose" = share ]; then expected=E_TYPE_BOUND; fi
        if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/${name}_${purpose}_rejected.seen" > "$TMP_DIR/$name-$purpose.log" 2>&1; then
            echo 'FAIL: staging owner or byte borrow escaped move or creator confinement' >&2; exit 1
        fi
        rg -Fq "$expected" "$TMP_DIR/$name-$purpose.log"
    done
done
echo 'PASS: staging metadata byte-borrow and Vulkan timeline feature foundations; native copy acceptance is separate'
