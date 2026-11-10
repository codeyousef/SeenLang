#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-io-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/completion-io-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=completion-io-a
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" contract "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/completion_contract.seen"
    for name in 'capacity ownership partial exactly-once and stale generation' \
        'preparing cancellation drain and idempotent close' \
        'aggregate byte budget rejection returns owners' \
        'deadline progress and generation exhaustion fail closed' \
        'deterministic cleanup live-allocation baseline'; do
        rg -Fq "PASS: IO-002A $name" "$TMP_DIR/contract-$profile.log"
    done
done
compile_case fast fuzz "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/completion_fuzz.seen"
rg -Fq 'PASS: IO-002A transition fuzz 60s seed=1101 counters ownership and duplicate rejection' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: backend-neutral bounded completion I/O ownership and state contracts'
