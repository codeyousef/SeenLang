#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-metal-c
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-metal-c.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-metal-c
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
# Certification evidence remains available on success as well as unsupported/failure.
SEEN_KEEP_TMP=1
python3 "$ROOT_DIR/tests/gpu_artifacts/certification_io_contract.py" "$TMP_DIR"
SOURCE="$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
cp -- "$SOURCE" "$TMP_DIR/original-source.seen"
for profile in fast release undefined address; do
    compile_case "$profile" artifact "$ROOT_DIR/tests/gpu_artifacts/integrity_contract.seen" metal "$SOURCE"
done
mkdir "$TMP_DIR/relocated"
cp -- "$SOURCE" "$TMP_DIR/relocated/source.seen"
for target in macos13 macos14 macos15; do
    for copy in original relocated; do
        source="$SOURCE"
        if [ "$copy" = relocated ]; then source="$TMP_DIR/relocated/source.seen"; fi
        run_compiler gpu emit "$source" --gpu-backend metal --gpu-target "$target" \
            --module-id gpu-artifact.seen --artifact-info --output "$TMP_DIR/$copy-$target.metal" \
            --reflection "$TMP_DIR/$copy-$target.json" > "$TMP_DIR/$copy-$target.log" 2>&1
    done
done
policy_args=()
if [ "${SEEN_METAL_TOOLCHAIN_POLICY+x}" = x ]; then
    policy_args=(--policy "$SEEN_METAL_TOOLCHAIN_POLICY")
fi
status=0
python3 "$ROOT_DIR/scripts/certify_gpu_metal.py" "$TMP_DIR" "${policy_args[@]}" || status=$?
if [ -f "$TMP_DIR/certification.json" ]; then cat -- "$TMP_DIR/certification.json"; fi
exit "$status"
