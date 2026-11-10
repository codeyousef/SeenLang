#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-io-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/completion-io-b.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=completion-io-b
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" contract "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/uring_contract.seen"
    for name in 'checked geometry SQ encoding wraparound cancellation and exactly-once CQ retirement' \
        'malformed CQ evidence retains owners and fails closed' 'mock ring lifetime baseline'; do
        rg -Fq "PASS: IO-002B $name" "$TMP_DIR/contract-$profile.log"
    done
    compile_case "$profile" native "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/uring_linux.seen" "$TMP_DIR/native-$profile.data"
    rg -Fq '"schema":"seen-io-uring-v1"' "$TMP_DIR/native-$profile.log"
    if [ "${SEEN_REQUIRE_IO_URING:-0}" = 1 ]; then
        rg -Fq '"classification":"verified"' "$TMP_DIR/native-$profile.log"
    fi
done
compile_case fast fuzz "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/uring_layout_fuzz.seen"
rg -Fq 'PASS: IO-002B layout fuzz 60s seed=1102 bounded maps and allocation baseline' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: io_uring contracts; native platform classification retained in fixture evidence'
