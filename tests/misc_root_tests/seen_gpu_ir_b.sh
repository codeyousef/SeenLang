#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ir-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ir-b.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ir-b
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" verifier "$ROOT_DIR/tests/typed_gpu_compiler/ir_verifier_contract.seen"
    compile_case "$profile" lowering "$ROOT_DIR/tests/typed_gpu_compiler/ir_lowering_contract.seen"
    compile_case "$profile" scalar "$ROOT_DIR/tests/typed_gpu_compiler/ir_scalar_contract.seen"
done
compile_case fast fuzz "$ROOT_DIR/tests/typed_gpu_compiler/ir_metadata_fuzz.seen"
echo 'PASS: structured GPU IR verifier contract'
