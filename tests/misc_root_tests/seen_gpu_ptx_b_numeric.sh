#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ptx-b-numeric
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ptx-b-numeric.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ptx-b-numeric
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/scripts/check_gpu_ptx_toolchain.py" > "$TMP_DIR/toolchain.json"
for profile in fast release undefined address; do
    compile_case "$profile" numeric "$ROOT_DIR/tests/gpu_scalar_words/ptx_numeric_contract.seen"
    rg -Fq 'PASS: emitted scalar PTX matches fixed IEEE packed goldens and allocation baseline' "$TMP_DIR/numeric-$profile.log"
    compile_case "$profile" specialization "$ROOT_DIR/tests/gpu_artifacts/specialization_contract.seen" cuda
    rg -Fq 'PASS: PTX bounded specialization overrides preserve source and independent clone ownership' "$TMP_DIR/specialization-$profile.log"
done
compile_case fast fuzz "$ROOT_DIR/tests/gpu_scalar_words/packed_fuzz.seen" cuda "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
rg -Fq 'PASS: packed capability fuzz 60s seed=1089 preserved typed rejection and artifact ownership' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: CUDA PTX scalar numeric specialization and packed artifact ownership contracts'
