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
    compile_case "$profile" numeric "$ROOT_DIR/tests/gpu_scalar_words/amdgpu_numeric_contract.seen"
    rg -Fq 'PASS: emitted AMDGPU scalar cases share fixed IEEE packed goldens and restore allocation baseline' "$TMP_DIR/numeric-$profile.log"
    compile_case "$profile" specialization "$ROOT_DIR/tests/gpu_artifacts/specialization_contract.seen" hip
    rg -Fq 'PASS: AMDGPU bounded specialization overrides preserve source and independent clone ownership' "$TMP_DIR/specialization-$profile.log"
    compile_case "$profile" rejection "$ROOT_DIR/tests/hip_amdgpu_lowering/capabilities_contract.seen"
    rg -Fq 'PASS: AMDGPU native CDNA precision shape layout full-wave and immutable-fragment rejection restore ownership' "$TMP_DIR/rejection-$profile.log"
    compile_fixture "$profile" text "$ROOT_DIR/tests/gpu_artifacts/text_layout_contract.seen"
    for pair in capabilities:cuda_ptx_lowering/capabilities_happy packed:spir_v_vulkan_lowering/packed_happy atomic:hip_amdgpu_lowering/atomic_happy subgroup:hip_amdgpu_lowering/subgroup_happy cooperative:hip_amdgpu_lowering/cooperative_happy; do
        name=${pair%%:*}; source="$ROOT_DIR/tests/${pair#*:}.seen"; arguments=()
        if [ "$name" = cooperative ]; then arguments=(cdna); fi
        cp -- "$TMP_DIR/text-$profile" "$TMP_DIR/$name-$profile"
        cp -- "$TMP_DIR/text-$profile.log" "$TMP_DIR/$name-$profile.log"
        run_fixture "$profile" "$name" hip "$source" "${arguments[@]}"
        rg -Fq 'PASS: AMDGPU target coverage deterministic artifacts reflection and repeated cleanup' "$TMP_DIR/$name-$profile.log"
    done
done
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_artifacts.py" "$TMP_DIR" capabilities packed atomic subgroup cooperative --cdna-only cooperative
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_capabilities.py" "$TMP_DIR/atomic-fast-gfx90a.ll"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_subgroups.py" "$TMP_DIR/subgroup-fast-gfx90a.ll" "$TMP_DIR/subgroup-fast-gfx1100.ll"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_cooperative.py" "$TMP_DIR/cooperative-fast-gfx90a.ll" "$TMP_DIR/cooperative-fast-gfx942.ll"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_native.py" "$TMP_DIR"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_numerics.py" "$TMP_DIR/numeric-fast.log"
python3 - "$ROOT_DIR/scripts" "$TMP_DIR" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from check_gpu_amdgpu_numerics import cases_from_log
root = Path(sys.argv[2]); baseline = cases_from_log(root / 'numeric-fast.log')
for profile in ['release', 'undefined', 'address']:
    assert cases_from_log(root / ('numeric-' + profile + '.log')) == baseline, 'numeric LLVM differs across profiles'
print('PASS: AMDGPU numeric LLVM and fixed corpus identity across four compiler profiles')
PY
compile_case fast fuzz "$ROOT_DIR/tests/gpu_scalar_words/packed_fuzz.seen" hip "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
rg -Fq 'PASS: packed capability fuzz 60s seed=1089 preserved typed rejection and artifact ownership' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: AMDGPU native capabilities scalar goldens specialized ownership subgroup oracles and checked MFMA fragments'
