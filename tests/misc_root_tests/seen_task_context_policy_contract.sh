#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-task-context-policy
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/task-context-policy.XXXXXX")"
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/task-context-policy.*) rm -rf -- "$TMP_DIR" ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved executor artifacts: $TMP_DIR" >&2
        seen_report_fixture_failure_logs "$TMP_DIR"
    fi
    exit "$status"
}
trap cleanup EXIT

compile_case() {
    local profile=$1
    local name=$2
    local source=$3
    local expected_status=${4:-0}
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    local log="$TMP_DIR/$name-$profile.log"
    bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$source" "$TMP_DIR/$name-$profile" \
        --no-cache --frozen --target-cpu=x86-64 \
        "${flags[@]}" >"$log" 2>&1
    local status=0
    if (ulimit -c 0; timeout --foreground --kill-after=5s 120s "$TMP_DIR/$name-$profile") >>"$log" 2>&1; then
        status=0
    else
        status=$?
    fi
    if [ "$status" -ne "$expected_status" ]; then
        echo "FAIL: $name-$profile exited $status, expected $expected_status" >&2
        return 1
    fi
}

for profile in fast release; do
    compile_case "$profile" lifecycle "$ROOT_DIR/seen_std/tests/async/async-002f.seen"
    compile_case "$profile" unhandled-panic "$ROOT_DIR/tests/fixtures/async/unhandled_task_panic.seen" 134
    rg -Fq "PANIC:" "$TMP_DIR/unhandled-panic-$profile.log"
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: ASYNC-002F_$case_name" "$TMP_DIR/lifecycle-$profile.log"
    done
done

if bash "$ATTESTED_SEEN" "$COMPILER" check \
    "$ROOT_DIR/tests/fixtures/async/escaping_task_context.seen" >"$TMP_DIR/unsafe.log" 2>&1; then
    echo "FAIL: executor admitted an escaping borrowed task context" >&2
    exit 1
fi
rg -Fq E_TYPE_BOUND "$TMP_DIR/unsafe.log"
echo "PASS: task capture, typed panic, progress and fallback policy in fast and release/ThinLTO"
