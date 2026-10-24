#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ptx-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ptx-b.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ptx-b
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/scripts/check_gpu_ptx_toolchain.py" > "$TMP_DIR/toolchain.json"
for profile in fast release undefined address; do
    # Numeric/specialization and packed fuzz run in their mandatory gate.
    compile_case "$profile" rejection "$ROOT_DIR/tests/cuda_ptx_lowering/capabilities_contract.seen"
    rg -Fq 'PASS: PTX native target shape layout and full-warp rejection preserve selected-entry isolation' "$TMP_DIR/rejection-$profile.log"
    compile_fixture "$profile" layout "$ROOT_DIR/tests/gpu_artifacts/text_layout_contract.seen"
    for pair in capabilities:cuda_ptx_lowering/capabilities_happy packed:spir_v_vulkan_lowering/packed_happy cooperative:cuda_ptx_lowering/cooperative_happy bf16:cuda_ptx_lowering/cooperative_bf16; do
        name=${pair%%:*}; source="$ROOT_DIR/tests/${pair#*:}.seen"; arguments=()
        if [ "$name" = bf16 ]; then arguments=(sm_80+); fi
        cp -- "$TMP_DIR/layout-$profile" "$TMP_DIR/$name-$profile"
        cp -- "$TMP_DIR/layout-$profile.log" "$TMP_DIR/$name-$profile.log"
        cp -- "$TMP_DIR/layout-$profile.compile.log" "$TMP_DIR/$name-$profile.compile.log"
        cp -- "$TMP_DIR/layout-$profile.compile.status" "$TMP_DIR/$name-$profile.compile.status"
        run_fixture "$profile" "$name" cuda "$source" "${arguments[@]}"
        rg -Fq 'PASS: PTX target coverage deterministic artifacts reflection and repeated cleanup' "$TMP_DIR/$name-$profile.log"
    done
done
python3 "$ROOT_DIR/scripts/check_gpu_ptx_artifacts.py" "$TMP_DIR" capabilities packed cooperative bf16 --sm80-minimum bf16
hardware_args=()
if [ "${SEEN_REQUIRE_PTX_HARDWARE:-0}" = 1 ]; then hardware_args+=(--require-device); fi
timeout --foreground --kill-after=5s 120s python3 "$ROOT_DIR/scripts/check_gpu_ptx_hardware.py" "$TMP_DIR" --kind capabilities --compiler "$COMPILER" "${hardware_args[@]}" > "$TMP_DIR/hardware-evidence.json"
cp "$TMP_DIR/hardware-evidence.json" "$SEEN_ARTIFACT_ROOT/ptx-b-hardware-evidence.json"
cat "$TMP_DIR/hardware-evidence.json"
echo 'PASS: CUDA PTX atomics subgroup and native cooperative capability contracts'
