#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
SCOPE=seen-json-tiny-object-regression

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi

COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
[ -f "$ASAN_RUNNER" ] && [ ! -L "$ASAN_RUNNER" ] || {
    echo "FAIL: FEL-507 bounded ASan runner is missing or unsafe" >&2
    exit 1
}
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-json-tiny-object.XXXXXX")"
FAILURE_TAIL_BYTES=32768

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-json-tiny-object.*)
                [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                    [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
                rm -rf -- "$TMP_DIR"
                ;;
            *) return 1 ;;
        esac
    else
        echo "KEEP: $TMP_DIR"
    fi
}

trap cleanup EXIT

SRC="$ROOT_DIR/tests/compiler_regressions/fel_507_json_tiny_objects.seen"
ALIAS_SRC="$ROOT_DIR/tests/compiler_regressions/fel_507_json_alias_rejection.seen"
TARGET_DIR="$TMP_DIR/target"
mkdir -p "$TARGET_DIR"
[ -f "$SRC" ] && [ ! -L "$SRC" ] || {
    echo "FAIL: FEL-507 tracked compiler regression is missing or unsafe" >&2
    exit 1
}
[ -f "$ALIAS_SRC" ] && [ ! -L "$ALIAS_SRC" ] || {
    echo "FAIL: FEL-507 alias-rejection regression is missing or unsafe" >&2
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
    local policy=$2
    local binary=$3
    local compile_log=$4
    local -a compile_args=(
        compile "$SRC" "$binary" --fast --no-cache
    )
    local status=0

    if [ -n "$policy" ]; then
        compile_args+=(--sanitize "$policy")
    fi
    timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" "${compile_args[@]}" \
        >"$compile_log" 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then
        return 0
    fi
    fail_with_log \
        "FEL-507 $label tiny-object JSON repro did not compile (status $status)" \
        "$compile_log"
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
            "FEL-507 $label tiny-object JSON repro exited with status $status" \
            "$run_log"
    fi
    if [ -n "$policy" ] && grep -Eiq -- \
        "ERROR: AddressSanitizer|SUMMARY: AddressSanitizer|UndefinedBehaviorSanitizer|runtime error:" \
        "$run_log"; then

        fail_with_log \
            "FEL-507 $label tiny-object JSON repro emitted a sanitizer diagnostic" \
            "$run_log"
    fi
}

NORMAL_BIN="$TARGET_DIR/json-tiny-object-normal"
NORMAL_COMPILE_LOG="$TMP_DIR/normal.compile.log"
NORMAL_RUN_LOG="$TMP_DIR/normal.run.log"
compile_variant normal "" "$NORMAL_BIN" "$NORMAL_COMPILE_LOG"
run_variant normal "" "$NORMAL_BIN" "$NORMAL_RUN_LOG" "$NORMAL_COMPILE_LOG"

for policy in undefined address; do
    SANITIZED_BIN="$TARGET_DIR/json-tiny-object-$policy"
    SANITIZED_COMPILE_LOG="$TMP_DIR/$policy.compile.log"
    SANITIZED_RUN_LOG="$TMP_DIR/$policy.run.log"
    compile_variant "$policy-sanitized" "$policy" \
        "$SANITIZED_BIN" "$SANITIZED_COMPILE_LOG"
    run_variant "$policy-sanitized" "$policy" \
        "$SANITIZED_BIN" "$SANITIZED_RUN_LOG" "$SANITIZED_COMPILE_LOG"
    echo "EVIDENCE: FEL-507 --sanitize $policy compile/run passed"
done

SRC="$ALIAS_SRC"
ALIAS_BIN="$TARGET_DIR/json-alias-rejection"
ALIAS_COMPILE_LOG="$TMP_DIR/alias.compile.log"
ALIAS_RUN_LOG="$TMP_DIR/alias.run.log"
compile_variant alias-rejection "" "$ALIAS_BIN" "$ALIAS_COMPILE_LOG"
alias_status=0
timeout --foreground --kill-after=5s 30s "$ALIAS_BIN" \
    >"$ALIAS_RUN_LOG" 2>&1 || alias_status=$?
if [ "$alias_status" -ne 134 ]; then

    fail_with_log \
        "FEL-507 corrupted alias graph did not abort with status 134 (status $alias_status)" \
        "$ALIAS_RUN_LOG"
fi
grep -Fq -- "JsonValue_destroy: duplicate or cyclic value" \
    "$ALIAS_RUN_LOG" || fail_with_log \
        "FEL-507 alias rejection lacked the stable ownership diagnostic" \
        "$ALIAS_RUN_LOG"
echo "EVIDENCE: FEL-507 duplicate/cyclic raw alias rejected before cleanup"

echo "PASS: FEL-507 tiny JSON objects, nested values, errors, and lifetimes remain usable"
