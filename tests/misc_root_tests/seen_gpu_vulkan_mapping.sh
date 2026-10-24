#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-vulkan-mapping
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-vulkan-mapping.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-vulkan-mapping
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
export SEEN_VULKAN_FIXTURE_MAP_TRACE=1
faults=(map-status map-empty map-address-overflow map-span-overflow map-not-visible map-invalidate map-flush-once map-noncoherent)
codes=(gpu.unavailable gpu.invalid gpu.invalid gpu.invalid gpu.unsupported_capability gpu.out_of_memory gpu.out_of_memory none)
traces=('map=1 unmap=0 flush=0 invalidate=0' 'map=1 unmap=1 flush=0 invalidate=0'
    'map=1 unmap=1 flush=0 invalidate=0' 'map=1 unmap=1 flush=0 invalidate=0' 'map=0 unmap=0 flush=0 invalidate=0'
    'map=1 unmap=1 flush=0 invalidate=1' 'map=2 unmap=2 flush=2 invalidate=1'
    'map=2 unmap=2 flush=1 invalidate=1')
run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/vulkan_mapping_contract.seen" > "$TMP_DIR/frontend.log" 2>&1
for profile in fast release undefined address; do
    compile_fixture "$profile" mapping "$ROOT_DIR/tests/production_gpu_runtime/vulkan_mapping_contract.seen"
    seen_run_vulkan_reference "$profile" mapping
    rg -Fq 'VULKAN_MAP_TRACE map=20 unmap=20 flush=0 invalidate=0' "$TMP_DIR/mapping-$profile.log"
    compile_fixture "$profile" mapping-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_mapping_fault_contract.seen"
    for index in "${!faults[@]}"; do
        seen_run_vulkan_fault "$profile" mapping-fault "${faults[index]}" "${codes[index]}"
        rg -Fq "VULKAN_MAP_TRACE ${traces[index]}" "$TMP_DIR/fault-mapping-fault-${faults[index]}-$profile.log"
    done
done
for purpose in move share; do
    expected=E_OWNERSHIP_USE_AFTER_MOVE
    if [ "$purpose" = share ]; then expected=E_TYPE_BOUND; fi
    if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/vulkan_mapping_${purpose}_rejected.seen" > "$TMP_DIR/$purpose-rejected.log" 2>&1; then
        echo 'FAIL: mapped host view escaped move or creator confinement' >&2; exit 1
    fi
    rg -Fq "$expected" "$TMP_DIR/$purpose-rejected.log"
done
echo 'PASS: Vulkan host mappings byte parity cache visibility failure retry native map counts and cold cleanup'
