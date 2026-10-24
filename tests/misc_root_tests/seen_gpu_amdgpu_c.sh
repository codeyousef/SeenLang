#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-amdgpu-c
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-amdgpu-c.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-amdgpu-c
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_toolchain.py" > "$TMP_DIR/toolchain.json"
python3 "$ROOT_DIR/tests/gpu_artifacts/certification_io_contract.py" "$TMP_DIR"
for profile in fast release undefined address; do
    compile_case "$profile" artifact "$ROOT_DIR/tests/gpu_artifacts/integrity_contract.seen" hip \
        "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
    rg -Fq 'PASS: GPU artifact hashes manifests mutation rejection and allocation baseline' "$TMP_DIR/artifact-$profile.log"
done
mkdir "$TMP_DIR/relocated"
cp "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen" "$TMP_DIR/relocated/source.seen"
for target in gfx90a gfx942 gfx1100 gfx1101 gfx1200; do
    for copy in original relocated; do
        source="$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
        if [ "$copy" = relocated ]; then source="$TMP_DIR/relocated/source.seen"; fi
        run_compiler gpu emit "$source" --gpu-backend hip --gpu-target "$target" \
            --module-id hip-artifact --artifact-info --output "$TMP_DIR/$copy-$target.ll" \
            --reflection "$TMP_DIR/$copy-$target.json" > "$TMP_DIR/$copy-$target.log" 2>&1
    done
done
python3 "$ROOT_DIR/scripts/certify_gpu_amdgpu.py" "$TMP_DIR"
