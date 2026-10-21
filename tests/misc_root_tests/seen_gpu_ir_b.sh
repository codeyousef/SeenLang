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
    for expectation in 'typed values owned regions and collective convergence' \
        'malformed identities access exits memory semantics and requirements rejected' \
        'verifier success and rejection release all owned allocations'; do
        rg -Fq "PASS: GPU-001B $expectation" "$TMP_DIR/verifier-$profile.log"
    done
done
compile_case fast fuzz "$ROOT_DIR/tests/typed_gpu_compiler/ir_metadata_fuzz.seen"
rg -Fq 'PASS: GPU-001B IR metadata fuzz 60s seed=1081 checked identities limits and allocation baseline' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: structured GPU IR verifier contract'
