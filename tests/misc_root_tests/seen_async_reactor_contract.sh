#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-reactor
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/async-reactor.XXXXXX")"
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/async-reactor.*) rm -rf -- "$TMP_DIR" ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved executor artifacts: $TMP_DIR" >&2
        seen_report_fixture_failure_logs "$TMP_DIR"
    fi
    exit "$status"
}
trap cleanup EXIT

REACTOR_COMPILE_FLAGS=()
compile_case() {
    local profile=$1
    local name=$2
    local source=$3
    shift 3
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    local log="$TMP_DIR/$name-$profile.log"
    bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$source" "$TMP_DIR/$name-$profile" \
        --no-cache --frozen --target-cpu=x86-64 \
        "${flags[@]}" "${REACTOR_COMPILE_FLAGS[@]}" >"$log" 2>&1
    timeout --foreground --kill-after=5s 120s "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1
}

for profile in fast release; do
    compile_case "$profile" lifecycle "$ROOT_DIR/seen_std/tests/async/async-002d.seen"
    for name in adverse deadlines sockets; do
        compile_case "$profile" "$name" "$ROOT_DIR/seen_std/tests/async/async-002d-$name.seen"
    done
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: ASYNC-002D_$case_name" "$TMP_DIR/lifecycle-$profile.log"
    done
done

REACTOR_COMPILE_FLAGS=(--coverage)
for name in lifecycle adverse deadlines sockets io; do
    source="$ROOT_DIR/seen_std/tests/async/async-002d-$name.seen"
    if [ "$name" = lifecycle ]; then source="$ROOT_DIR/seen_std/tests/async/async-002d.seen"; fi
    if [ "$name" = io ]; then source="$ROOT_DIR/seen_std/tests/async/io_completion_stress.seen"; fi
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    compile_case fast "coverage-$name" "$source" "$TMP_DIR/completion.bin"
done
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_verify_source_coverage "$TMP_DIR/reactor" \
    --fixture "$TMP_DIR/coverage-lifecycle-fast" "$TMP_DIR/lifecycle.profraw" \
    --fixture "$TMP_DIR/coverage-adverse-fast" "$TMP_DIR/adverse.profraw" \
    --fixture "$TMP_DIR/coverage-deadlines-fast" "$TMP_DIR/deadlines.profraw" \
    --fixture "$TMP_DIR/coverage-sockets-fast" "$TMP_DIR/sockets.profraw" \
    --fixture "$TMP_DIR/coverage-io-fast" "$TMP_DIR/io.profraw" \
    --source seen_std/src/async/event_loop.seen \
    --source seen_std/src/async/deadline_heap.seen \
    --source seen_std/src/async/sockets.seen \
    --source seen_std/src/async/io_lease.seen \
    --source seen_std/src/async/io_wakeup.seen

if bash "$ATTESTED_SEEN" "$COMPILER" check \
    "$ROOT_DIR/tests/fixtures/async/unsafe_io_job.seen" >"$TMP_DIR/unsafe.log" 2>&1; then
    echo "FAIL: reactor admitted a borrowed raw callback" >&2
    exit 1
fi
rg -Fq E_TYPE_BOUND "$TMP_DIR/unsafe.log"
echo "PASS: bounded reactor dispatch, timers and sockets in fast and release/ThinLTO"
