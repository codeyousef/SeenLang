#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-spirv-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-spirv-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-spirv-a
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
command -v spirv-val >/dev/null
spirv-val --version > "$TMP_DIR/validator-version.log"
for profile in fast release undefined address; do
    compile_case "$profile" primitive "$ROOT_DIR/tests/spir_v_vulkan_lowering/primitive_contract.seen"
    compile_case "$profile" emitter "$ROOT_DIR/tests/spir_v_vulkan_lowering/emitter_contract.seen"
    rg -Fq 'PASS: GPU-002A resource helper control flow deterministic reflection and partial cleanup' "$TMP_DIR/emitter-$profile.log"
done
python3 "$ROOT_DIR/scripts/check_gpu_spirv_artifacts.py" "$TMP_DIR" primitive emitter

run_compiler gpu emit "$ROOT_DIR/tests/typed_gpu_compiler/reflection_happy.seen" \
    --gpu-backend spirv --gpu-target vulkan1.2 --output "$TMP_DIR/cli.spv" --reflection "$TMP_DIR/cli.json" > "$TMP_DIR/cli.log" 2>&1
spirv-val --target-env vulkan1.2 "$TMP_DIR/cli.spv"
run_compiler gpu reflect "$ROOT_DIR/tests/typed_gpu_compiler/reflection_happy.seen" \
    --gpu-backend spirv --gpu-target vulkan1.2 --output "$TMP_DIR/reflection.json" > "$TMP_DIR/reflection.log" 2>&1
cmp "$TMP_DIR/cli.json" "$TMP_DIR/reflection.json"
for target in vulkan1.2 vulkan1.3; do
    run_compiler gpu emit "$ROOT_DIR/tests/spir_v_vulkan_lowering/return_control.seen" \
        --gpu-backend spirv --gpu-target "$target" --output "$TMP_DIR/returns-$target.spv" \
        > "$TMP_DIR/returns-$target.log" 2>&1
    spirv-val --target-env "$target" "$TMP_DIR/returns-$target.spv"
done
status=0
run_compiler gpu emit "$ROOT_DIR/tests/typed_gpu_compiler/rejection_host.seen" \
    --gpu-backend spirv --output "$TMP_DIR/rejected.spv" > "$TMP_DIR/rejected.log" 2>&1 || status=$?
test "$status" -eq 1
rg -Fq E_GPU_UNSUPPORTED "$TMP_DIR/rejected.log"
test ! -e "$TMP_DIR/rejected.spv"
compile_case fast fuzz "$ROOT_DIR/tests/spir_v_vulkan_lowering/emitter_fuzz.seen"
rg -Fq 'PASS: GPU-002A emission fuzz 60s seed=1088 typed rejection and allocation baseline' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: SPIR-V Vulkan layouts entries control flow official validation and reflection parity'
