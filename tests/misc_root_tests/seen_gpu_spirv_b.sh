#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-spirv-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-spirv-b.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-spirv-b
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
command -v spirv-val >/dev/null
spirv-val --version > "$TMP_DIR/validator-version.log"
for profile in fast release undefined address; do
    compile_case "$profile" numeric "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_numeric_contract.seen"
    rg -Fq 'PASS: GPU-002B emitted scalar SPIR-V matches fixed IEEE packed goldens and allocation baseline' "$TMP_DIR/numeric-$profile.log"
    compile_case "$profile" capabilities "$ROOT_DIR/tests/spir_v_vulkan_lowering/capabilities_contract.seen"
    rg -Fq 'PASS: GPU-002B atomic subgroup specialization native cooperative formats and typed rejection' "$TMP_DIR/capabilities-$profile.log"
    compile_case "$profile" packed "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_contract.seen" "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
    rg -Fq 'PASS: GPU-002B all packed formats dynamic lanes scale encoding and cleanup' "$TMP_DIR/packed-$profile.log"
done
python3 "$ROOT_DIR/scripts/check_gpu_spirv_artifacts.py" "$TMP_DIR" capabilities packed \
    --require-opcodes 12,50,201,202,203,224,225,227,228,229,230,234,235,236,238,240,241,242,333,334,335,339,341,345,349,4456,4457,4458,4459 \
    --require-capabilities 1,9,39,61,62,63,64,65,6022
for pair in native:E_GPU_SPIRV_CAPABILITY order:E_GPU_SPIRV_MEMORY; do
    name=${pair%:*}; code=${pair#*:}; status=0
    run_compiler gpu emit "$ROOT_DIR/tests/spir_v_vulkan_lowering/unsupported_$name.seen" \
        --gpu-backend spirv --gpu-fallback cpu --output "$TMP_DIR/rejected-$name.spv" \
        > "$TMP_DIR/rejected-$name.log" 2>&1 || status=$?
    test "$status" -eq 1
    rg -Fq "$code" "$TMP_DIR/rejected-$name.log"
    test ! -e "$TMP_DIR/rejected-$name.spv"
done
compile_case fast fuzz "$ROOT_DIR/tests/spir_v_vulkan_lowering/capabilities_fuzz.seen" "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
rg -Fq 'PASS: GPU-002B capability fuzz 60s seed=1089 preserved typed rejection and ownership' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: SPIR-V capability lowering numeric goldens official validation reflection and rejection fuzz'
