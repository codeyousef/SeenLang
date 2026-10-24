#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-amdgpu-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-amdgpu-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-amdgpu-a
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_toolchain.py" > "$TMP_DIR/toolchain.json"
for profile in fast release undefined address; do
    compile_fixture "$profile" text "$ROOT_DIR/tests/gpu_artifacts/text_layout_contract.seen"
    for pair in layout:cuda_ptx_lowering/layout_happy edges:hip_amdgpu_lowering/layout_edges scalar:hip_amdgpu_lowering/scalar_differential splat:gpu_artifacts/vector_splat; do
        name=${pair%%:*}; source="$ROOT_DIR/tests/${pair#*:}.seen"
        cp -- "$TMP_DIR/text-$profile" "$TMP_DIR/$name-$profile"
        cp -- "$TMP_DIR/text-$profile.log" "$TMP_DIR/$name-$profile.log"
        cp -- "$TMP_DIR/text-$profile.compile.log" "$TMP_DIR/$name-$profile.compile.log"
        cp -- "$TMP_DIR/text-$profile.compile.status" "$TMP_DIR/$name-$profile.compile.status"
        run_fixture "$profile" "$name" hip "$source"
        rg -Fq 'PASS: AMDGPU target coverage deterministic artifacts reflection and repeated cleanup' "$TMP_DIR/$name-$profile.log"
    done
    compile_case "$profile" admission "$ROOT_DIR/tests/hip_amdgpu_lowering/admission_contract.seen"
    rg -Fq 'PASS: AMDGPU selected-entry capabilities padded shared limits implicit geometry and bounded identities restore ownership' "$TMP_DIR/admission-$profile.log"
done
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_artifacts.py" "$TMP_DIR" layout edges scalar splat
python3 "$ROOT_DIR/tests/gpu_artifacts/amdgpu_metadata_contract.py" "$TMP_DIR"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_scalar.py" "$TMP_DIR/scalar-fast-gfx90a.ll" "$TMP_DIR/edges-fast-gfx90a.ll" "$TMP_DIR/splat-fast-gfx90a.ll"
compile_case fast rejection "$ROOT_DIR/tests/gpu_artifacts/text_rejection_contract.seen" hip
rg -Fq 'PASS: AMDGPU 60s seed=1094 malformed IR target capability and padded shared limits restore allocation baseline' "$TMP_DIR/rejection-fast.log"
echo 'PASS: HIP AMDGPU layout reflected HSA ABI official code objects host scalar differential and bounded ownership'
