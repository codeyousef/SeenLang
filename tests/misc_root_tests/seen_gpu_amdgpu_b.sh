#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-amdgpu-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-amdgpu-b.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-amdgpu-b
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_toolchain.py" > "$TMP_DIR/toolchain.json"
for profile in fast release undefined address; do
    # Numeric/specialization parity has its own mandatory four-profile gate.
    compile_case "$profile" rejection "$ROOT_DIR/tests/hip_amdgpu_lowering/capabilities_contract.seen"
    rg -Fq 'PASS: AMDGPU native CDNA precision shape layout full-wave and immutable-fragment rejection restore ownership' "$TMP_DIR/rejection-$profile.log"
    compile_fixture "$profile" text "$ROOT_DIR/tests/gpu_artifacts/text_layout_contract.seen"
    for pair in capabilities:cuda_ptx_lowering/capabilities_happy packed:spir_v_vulkan_lowering/packed_happy atomic:hip_amdgpu_lowering/atomic_happy subgroup:hip_amdgpu_lowering/subgroup_happy cooperative:hip_amdgpu_lowering/cooperative_happy; do
        name=${pair%%:*}; source="$ROOT_DIR/tests/${pair#*:}.seen"; arguments=()
        if [ "$name" = cooperative ]; then arguments=(cdna); fi
        cp -- "$TMP_DIR/text-$profile" "$TMP_DIR/$name-$profile"
        cp -- "$TMP_DIR/text-$profile.log" "$TMP_DIR/$name-$profile.log"
        cp -- "$TMP_DIR/text-$profile.compile.log" "$TMP_DIR/$name-$profile.compile.log"
        cp -- "$TMP_DIR/text-$profile.compile.status" "$TMP_DIR/$name-$profile.compile.status"
        run_fixture "$profile" "$name" hip "$source" "${arguments[@]}"
        rg -Fq 'PASS: AMDGPU target coverage deterministic artifacts reflection and repeated cleanup' "$TMP_DIR/$name-$profile.log"
    done
done
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_artifacts.py" "$TMP_DIR" capabilities packed atomic subgroup cooperative --cdna-only cooperative
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_capabilities.py" "$TMP_DIR/atomic-fast-gfx90a.ll"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_subgroups.py" "$TMP_DIR/subgroup-fast-gfx90a.ll" "$TMP_DIR/subgroup-fast-gfx1100.ll"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_cooperative.py" "$TMP_DIR/cooperative-fast-gfx90a.ll" "$TMP_DIR/cooperative-fast-gfx942.ll"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_native.py" "$TMP_DIR"
# Packed numeric fuzz is mandatory in seen_gpu_amdgpu_b_numeric.sh.
echo 'PASS: AMDGPU capability artifacts subgroup oracles and MFMA instructions; device execution unverified'
