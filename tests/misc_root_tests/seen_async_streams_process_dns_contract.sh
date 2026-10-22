#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-streams-process-dns
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/async-streams-process-dns.XXXXXX")"
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/async-streams-process-dns.*) rm -rf -- "$TMP_DIR" ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved executor artifacts: $TMP_DIR" >&2
        seen_report_fixture_failure_logs "$TMP_DIR"
    fi
    exit "$status"
}
trap cleanup EXIT

STREAMS_COMPILE_FLAGS=()
compile_case() {
    local profile=$1
    local name=$2
    local source=$3
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    local log="$TMP_DIR/$name-$profile.log"
    bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$source" "$TMP_DIR/$name-$profile" \
        --no-cache --frozen --target-cpu=x86-64 \
        "${flags[@]}" "${STREAMS_COMPILE_FLAGS[@]}" >"$log" 2>&1
    timeout --foreground --kill-after=5s 120s "$TMP_DIR/$name-$profile" >>"$log" 2>&1
}

for profile in fast release; do
    compile_case "$profile" lifecycle "$ROOT_DIR/seen_std/tests/async/async-002e.seen"
    compile_case "$profile" arguments "$ROOT_DIR/tests/fixtures/async/process_arguments_invalid.seen"
    compile_case "$profile" dns-fuzz "$ROOT_DIR/seen_std/tests/async/dns_wire_fuzz.seen"
    for source in dns_wire_adverse dns_policy_adverse streams_adverse processes_adverse process_validation_corpus; do
        compile_case "$profile" "$source" "$ROOT_DIR/seen_std/tests/async/$source.seen"
    done
    rg -Fq "PASS: DNS wire fuzz 60s seed 1101" "$TMP_DIR/dns-fuzz-$profile.log"
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: ASYNC-002E_$case_name" "$TMP_DIR/lifecycle-$profile.log"
    done
done

STREAMS_COMPILE_FLAGS=(--coverage)
for name in lifecycle arguments dns-fuzz dns_wire_adverse dns_policy_adverse streams_adverse processes_adverse process_validation_corpus; do
    source="$ROOT_DIR/seen_std/tests/async/$name.seen"
    if [ "$name" = lifecycle ]; then source="$ROOT_DIR/seen_std/tests/async/async-002e.seen"; fi
    if [ "$name" = arguments ]; then source="$ROOT_DIR/tests/fixtures/async/process_arguments_invalid.seen"; fi
    if [ "$name" = dns-fuzz ]; then source="$ROOT_DIR/seen_std/tests/async/dns_wire_fuzz.seen"; fi
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    compile_case fast "coverage-$name" "$source"
done
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/streams" \
    --fixture "$TMP_DIR/coverage-lifecycle-fast" "$TMP_DIR/lifecycle.profraw" \
    --fixture "$TMP_DIR/coverage-arguments-fast" "$TMP_DIR/arguments.profraw" \
    --fixture "$TMP_DIR/coverage-dns-fuzz-fast" "$TMP_DIR/dns-fuzz.profraw" \
    --fixture "$TMP_DIR/coverage-dns_wire_adverse-fast" "$TMP_DIR/dns_wire_adverse.profraw" \
    --fixture "$TMP_DIR/coverage-dns_policy_adverse-fast" "$TMP_DIR/dns_policy_adverse.profraw" \
    --fixture "$TMP_DIR/coverage-streams_adverse-fast" "$TMP_DIR/streams_adverse.profraw" \
    --fixture "$TMP_DIR/coverage-processes_adverse-fast" "$TMP_DIR/processes_adverse.profraw" \
    --fixture "$TMP_DIR/coverage-process_validation_corpus-fast" "$TMP_DIR/process_validation_corpus.profraw" \
    --source seen_std/src/async/streams.seen \
    --source seen_std/src/async/processes.seen \
    --source seen_std/src/async/process_arguments.seen \
    --source seen_std/src/async/dns.seen \
    --source seen_std/src/async/dns_wire.seen
for parser in dns_wire process_arguments; do
    python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" \
        "$TMP_DIR/streams-coverage.json" --source "seen_std/src/async/$parser.seen" \
        --min-branches 90 >"$TMP_DIR/$parser-parser-coverage-summary.json"
done

echo "PASS: bounded streams, child processes and DNS in fast and release/ThinLTO"
