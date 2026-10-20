#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-io-c
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/completion-io-c.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=completion-io-c
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" contract "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/iocp_contract.seen"
    for name in 'offset encoding completion identity cancellation and stale generations' \
        'malformed progress retains record and buffer ownership' 'repeated lifecycle allocation baseline'; do
        rg -Fq "PASS: IO-002C $name" "$TMP_DIR/contract-$profile.log"
    done
    compile_case "$profile" native "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/iocp_windows.seen" "$TMP_DIR/native-$profile.data"
    rg -Fq '"schema":"seen-io-iocp-v1"' "$TMP_DIR/native-$profile.log"
    if [ "${SEEN_REQUIRE_IOCP:-0}" = 1 ]; then
        rg -Fq '"classification":"verified"' "$TMP_DIR/native-$profile.log"
    fi
done
compile_case fast fuzz "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/iocp_fuzz.seen"
rg -Fq 'PASS: IO-002C completion fuzz 60s seed=1103 identity progress and ownership' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: overlapped completion contracts; platform classification retained in fixture evidence'
