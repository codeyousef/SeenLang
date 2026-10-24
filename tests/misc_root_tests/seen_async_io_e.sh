#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-io-e
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/completion-io-e.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=completion-io-e
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" boolean-literals "$ROOT_DIR/tests/compiler_regressions/boolean_array_literals.seen"
    rg -Fq 'PASS: Boolean array literals preserve values evaluation and ownership' "$TMP_DIR/boolean-literals-$profile.log"
    compile_case "$profile" contract "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/adverse_completion.seen"
    for name in 'backend error matrix partial EOF and first-observed terminal reason' \
        'device event does not acknowledge preparing or native borrows' 'late acknowledgement allocation baseline'; do
        rg -Fq "PASS: IO-002E $name" "$TMP_DIR/contract-$profile.log"
    done
    rg -Fq '"schema":"seen-io-adverse-v1"' "$TMP_DIR/contract-$profile.log"
done
compile_case fast fuzz "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/device_fuzz.seen"
rg -Fq 'PASS: IO-002E device-loss fuzz 60s seed=1105 mixed phases first reason and ownership' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: shared adverse completion and device-removal contracts'
