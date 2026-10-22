#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-runtime-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-runtime-a
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fixtures=(backend error limits device_info discovery operation device device_stress)
for profile in fast release undefined address; do
    for name in "${fixtures[@]}"; do
        compile_case "$profile" "$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
        rg -Fq 'PASS: GPU-006A' "$TMP_DIR/$name-$profile.log"
    done
done
# Metadata and CPU-default imports must not discover or link an accelerator SDK.
readelf -d "$TMP_DIR/backend-fast" > "$TMP_DIR/cpu-default-libraries.txt"
if rg -i 'NEEDED.*(cuda|cudart|vulkan|amdhip|metal)' "$TMP_DIR/cpu-default-libraries.txt"; then
    echo 'FAIL: CPU-default metadata import linked an accelerator SDK' >&2
    exit 1
fi
FIXTURE_COMPILE_FLAGS=(--coverage)
coverage=()
for name in "${fixtures[@]}"; do
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    compile_case fast "coverage-$name" "$ROOT_DIR/tests/production_gpu_runtime/${name}_contract.seen"
    coverage+=(--fixture "$TMP_DIR/coverage-$name-fast" "$TMP_DIR/$name.profraw")
done
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/foundations" "${coverage[@]}" \
    --source seen_std/src/compute/backend.seen \
    --source seen_std/src/compute/error.seen \
    --source seen_std/src/compute/limits.seen \
    --source seen_std/src/compute/capabilities.seen \
    --source seen_std/src/compute/device_info.seen \
    --source seen_std/src/compute/discovery.seen \
    --source seen_std/src/compute/operation.seen \
    --source seen_std/src/compute/provider.seen \
    --source seen_std/src/compute/device.seen
for name in device context; do
    if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/${name}_move_rejected.seen" > "$TMP_DIR/$name-move.log" 2>&1; then
        echo 'FAIL: GPU ownership transfer allowed use of the previous owner' >&2
        exit 1
    fi
    rg -Fq E_OWNERSHIP_USE_AFTER_MOVE "$TMP_DIR/$name-move.log"
    if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/${name}_share_rejected.seen" > "$TMP_DIR/$name-share.log" 2>&1; then
        echo 'FAIL: GPU owner was implicitly admitted to cross-thread sharing' >&2
        exit 1
    fi
    rg -Fq E_TYPE_BOUND "$TMP_DIR/$name-share.log"
done
FIXTURE_COMPILE_FLAGS=()
unset LLVM_PROFILE_FILE
compile_case fast metadata-fuzz "$ROOT_DIR/tests/production_gpu_runtime/metadata_fuzz.seen"
rg -Fq 'PASS: GPU-006A metadata fuzz 60s seed=1100 preserves typed diagnostics bounds and ownership' "$TMP_DIR/metadata-fuzz-fast.log"
# Native execution deliberately selects the software reference driver. This is
# not hardware certification and production callers must opt into software use.
reference_icd=
for candidate in /usr/share/vulkan/icd.d/lvp_icd.json /usr/share/vulkan/icd.d/lvp_icd.x86_64.json; do
    if [ -f "$candidate" ] && [ ! -L "$candidate" ]; then reference_icd=$candidate; break; fi
done
[ -n "$reference_icd" ] || { echo 'FAIL: explicit Mesa Vulkan reference ICD is unavailable' >&2; exit 1; }
export VK_DRIVER_FILES="$reference_icd" LP_NUM_THREADS=1
# Compile the SDK oracle independently of the Seen layout declarations.
timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror \
    "$ROOT_DIR/tests/production_gpu_runtime/vulkan_abi_oracle.c" -o "$TMP_DIR/vulkan-abi-oracle"
"$TMP_DIR/vulkan-abi-oracle" > "$TMP_DIR/vulkan-abi-oracle.log"
timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -Wall -Wextra -Werror \
    -fPIC -shared "$ROOT_DIR/tests/production_gpu_runtime/vulkan_fault_driver.c" -ldl \
    -o "$TMP_DIR/vulkan-fault-driver.so"
fault_modes=(version-status version-old instance-status instance-empty devices-count-status devices-limit devices-fill-status devices-overflow queue-limit queue-overflow queue-none context-status context-empty)
fault_codes=(gpu.unavailable gpu.unsupported_capability gpu.out_of_memory gpu.invalid gpu.device_lost gpu.limit gpu.invalid gpu.invalid gpu.limit gpu.invalid gpu.unsupported_capability gpu.out_of_memory gpu.invalid)
run_vendor_faults() {
    local profile=$1 name=$2 index
    for index in "${!fault_modes[@]}"; do
        export SEEN_VULKAN_FIXTURE_FAULT="${fault_modes[index]}" LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so"
        export LLVM_PROFILE_FILE="$TMP_DIR/fault-${fault_modes[index]}.profraw"
        run_fixture "$profile" "$name" "${fault_modes[index]}" "${fault_codes[index]}"
        unset LD_PRELOAD SEEN_VULKAN_FIXTURE_FAULT
        cp -- "$TMP_DIR/$name-$profile.log" "$TMP_DIR/fault-${fault_modes[index]}-$profile.log"
        rg -Fq 'PASS: Vulkan vendor fault' "$TMP_DIR/fault-${fault_modes[index]}-$profile.log"
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
llvm-profdata merge -sparse "$TMP_DIR"/fault-*.profraw -o "$TMP_DIR/fault.profdata"
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
    --source seen_std/src/compute/vulkan_provider.seen \
    --source seen_std/src/compute/vulkan_queue.seen
python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" "$TMP_DIR/vulkan-coverage.json" \
    --source seen_std/src/compute/vulkan_properties.seen --min-lines 90 --min-branches 90 \
    > "$TMP_DIR/vulkan-parser-coverage-summary.json"
echo 'PASS: bounded GPU ownership and explicit Vulkan software reference discovery; hardware execution unverified'
