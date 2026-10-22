#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ir-c
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ir-c.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ir-c
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" precision "$ROOT_DIR/tests/typed_gpu_compiler/ir_precision_contract.seen"
    for expectation in 'closed packed precision and matrix descriptors' \
        'packed integer golden words neighboring bits faults and cleanup' \
        'explicit floating word conversions and MXFP4 scales' \
        'bounded specialization domains explicit overrides and cleanup' \
        'cooperative shape precision convergence and target admission'; do
        rg -Fq "PASS: GPU-001C $expectation" "$TMP_DIR/precision-$profile.log"
    done
done
run_compiler check "$ROOT_DIR/tests/typed_gpu_compiler/precision_happy.seen" --frozen > "$TMP_DIR/source-check.log" 2>&1
rg -Fq '[OK] Check passed' "$TMP_DIR/source-check.log"
for specification in cpu_rejected:E009 unknown_call:E_GPU_UNSUPPORTED; do
    name=${specification%:*}; code=${specification#*:}; status=0
    run_compiler check "$ROOT_DIR/tests/typed_gpu_compiler/precision_$name.seen" --frozen > "$TMP_DIR/$name.log" 2>&1 || status=$?
    test "$status" -eq 1
    rg -Fq "$code" "$TMP_DIR/$name.log"
done
compile_case fast fuzz "$ROOT_DIR/tests/typed_gpu_compiler/ir_precision_fuzz.seen"
rg -Fq 'PASS: GPU-001C precision metadata fuzz 60s seed=1084 rejected malformed profiles and overrides without leaks' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: GPU precision and cooperative matrix IR contract'
