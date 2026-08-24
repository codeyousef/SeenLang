#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
SCOPE=seen-nested-array-batch-regression

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi

COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
[ -f "$ASAN_RUNNER" ] && [ ! -L "$ASAN_RUNNER" ] || {
    echo "FAIL: FEL-544 bounded ASan runner is missing or unsafe" >&2
    exit 1
}
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-nested-array-batch.XXXXXX")"

cleanup() {
    local status=$?

    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-nested-array-batch.*)
                if [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                    [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ]; then
                    rm -rf -- "$TMP_DIR" || status=1
                else
                    status=1
                fi
                ;;
            *) status=1 ;;
        esac
    else
        echo "KEEP: $TMP_DIR" >&2
    fi

    trap - EXIT
    exit "$status"
}

trap cleanup EXIT

SRC="$ROOT_DIR/tests/compiler_regressions/fel_544_nested_array_batch.seen"
TARGET_DIR="$TMP_DIR/target"
FAILURE_TAIL_BYTES=32768
mkdir -p "$TARGET_DIR"

[ -f "$SRC" ] && [ ! -L "$SRC" ] || {
    echo "FAIL: FEL-544 tracked compiler regression is missing or unsafe" >&2
    exit 1
}

show_failure_tail() {
    local log_file=$1

    if [ -s "$log_file" ]; then
        echo "--- bounded failure tail: $log_file (at most $FAILURE_TAIL_BYTES bytes) ---" >&2
        tail -c "$FAILURE_TAIL_BYTES" -- "$log_file" >&2
        echo >&2
    else
        echo "--- no diagnostic output: $log_file ---" >&2
    fi
}

fail_with_log() {
    local message=$1
    local log_file=$2

    echo "FAIL: $message" >&2
    show_failure_tail "$log_file"
    exit 1
}

compile_variant() {
    local label=$1
    local binary=$2
    local compile_log=$3
    shift 3
    local status=0

    timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile "$SRC" "$binary" \
        --no-cache "$@" >"$compile_log" 2>&1 || status=$?
    if [ "$status" -ne 0 ]; then
        fail_with_log \
            "FEL-544 $label nested-array batch repro did not compile (status $status)" \
            "$compile_log"
    fi
    if [ ! -x "$binary" ] || [ -L "$binary" ]; then
        fail_with_log \
            "FEL-544 $label compile did not create a safe executable" \
            "$compile_log"
    fi
}

run_variant() {
    local label=$1
    local policy=$2
    local binary=$3
    local run_log=$4
    local compile_log=$5
    local status=0

    if [ "$policy" = "address" ]; then
        timeout --foreground --kill-after=5s 60s \
            bash "$ASAN_RUNNER" --target-root "$TARGET_DIR" \
            --compile-log "$compile_log" -- "$binary" \
            >"$run_log" 2>&1 || status=$?
    else
        timeout --foreground --kill-after=5s 60s \
            env UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
                "$binary" >"$run_log" 2>&1 || status=$?
    fi
    if [ "$status" -ne 0 ]; then
        fail_with_log \
            "FEL-544 $label nested-array batch repro exited with status $status" \
            "$run_log"
    fi
    if grep -Eiq -- \
        "ERROR: AddressSanitizer|SUMMARY: AddressSanitizer|UndefinedBehaviorSanitizer|runtime error:" \
        "$run_log"; then

        fail_with_log \
            "FEL-544 $label nested-array batch repro emitted a sanitizer diagnostic" \
            "$run_log"
    fi
}

OPTIMIZED_BIN="$TARGET_DIR/nested-array-batch-optimized"
OPTIMIZED_COMPILE_LOG="$TMP_DIR/optimized.compile.log"
OPTIMIZED_RUN_LOG="$TMP_DIR/optimized.run.log"
compile_variant optimized "$OPTIMIZED_BIN" "$OPTIMIZED_COMPILE_LOG"
if ! grep -Fq -- "[Pass 2b] Optimization complete" "$OPTIMIZED_COMPILE_LOG"; then
    fail_with_log \
        "FEL-544 optimized compile did not execute the optimizer path" \
        "$OPTIMIZED_COMPILE_LOG"
fi
run_variant optimized "" "$OPTIMIZED_BIN" "$OPTIMIZED_RUN_LOG" \
    "$OPTIMIZED_COMPILE_LOG"

# The primary Linux compiler contract declares both sanitizer policies
# supported. Any compile or runtime failure is therefore release-blocking.
for policy in undefined address; do
    SANITIZED_BIN="$TARGET_DIR/nested-array-batch-$policy"
    SANITIZED_COMPILE_LOG="$TMP_DIR/$policy.compile.log"
    SANITIZED_RUN_LOG="$TMP_DIR/$policy.run.log"
    compile_variant "$policy-sanitized" "$SANITIZED_BIN" \
        "$SANITIZED_COMPILE_LOG" --fast --sanitize "$policy"
    run_variant "$policy-sanitized" "$policy" \
        "$SANITIZED_BIN" "$SANITIZED_RUN_LOG" "$SANITIZED_COMPILE_LOG"
done

echo "PASS: FEL-544 nested Array batches pass optimized, UBSan, and ASan lifetime coverage"
