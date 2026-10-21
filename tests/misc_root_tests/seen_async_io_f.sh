#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-io-f
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/completion-io-f.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=completion-io-f
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/tests/misc_root_tests/seen_io_overlap_evidence_contract.py"
for profile in fast release undefined address; do
    compile_case "$profile" diagnostics "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/diagnostics_contract.seen"
    rg -Fq 'PASS: IO-002F fixed diagnostics snapshot lifetime saturation and 1000 exact cleanup rounds' "$TMP_DIR/diagnostics-$profile.log"
    compile_fixture "$profile" overlap "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/io_overlap.seen"
    command=("$TMP_DIR/overlap-$profile")
    if [ "$profile" = address ]; then
        command=(bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" --target-root "$TMP_DIR" \
            --compile-log "$TMP_DIR/overlap-$profile.log" -- "${command[@]}")
    fi
    UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
        python3 "$ROOT_DIR/scripts/capture_io_overlap.py" --output "$TMP_DIR/overlap-$profile.json" \
        --source "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/io_overlap.seen" --compiler "$COMPILER" \
        --executable "$TMP_DIR/overlap-$profile" --profile "$profile" -- "${command[@]}" >>"$TMP_DIR/overlap-$profile.log" 2>&1
    python3 "$ROOT_DIR/scripts/io_overlap_evidence.py" "$TMP_DIR/overlap-$profile.json"
done
EVIDENCE_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/io-overlap-evidence.XXXXXX")"
cp -- "$TMP_DIR"/overlap-*.json "$EVIDENCE_DIR/"
echo 'PASS: bounded completion diagnostics and observed kernel-pipe I/O compute overlap'
