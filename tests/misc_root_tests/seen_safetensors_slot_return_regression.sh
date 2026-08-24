#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
FIXTURE="$ROOT_DIR/tests/compiler_regressions/fel_543_safetensors_slot_return.seen"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
SCOPE=seen-safetensors-slot-return-regression

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi

COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
[ -f "$ASAN_RUNNER" ] && [ ! -L "$ASAN_RUNNER" ] || {
    echo "FAIL: FEL-543 bounded ASan runner is missing or unsafe" >&2
    exit 1
}
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-safetensors-slot-return.XXXXXX")"

cleanup() {
    local status=$?
    if [ -n "${SEEN_KEEP_TMP:-}" ]; then
        echo "KEEP: $TMP_DIR" >&2
    else
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-safetensors-slot-return.*)
                if [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                    [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ]; then

                    rm -rf -- "$TMP_DIR"
                else
                    echo "ERROR: refusing to clean unsafe FEL-543 path: $TMP_DIR" >&2
                    status=1
                fi
                ;;
            *)
                echo "ERROR: refusing to clean unexpected FEL-543 path: $TMP_DIR" >&2
                status=1
                ;;
        esac
    fi
    trap - EXIT
    exit "$status"
}
trap cleanup EXIT

failure_tail() {
    local label=$1
    local log=$2
    local status=$3

    echo "FAIL: $label exited with status $status" >&2
    tail -c 32768 -- "$log" >&2 || true
}

sanitizer_reported_failure() {
    local log=$1

    grep -Eiq \
        'ERROR: (AddressSanitizer|LeakSanitizer)|SUMMARY: (AddressSanitizer|UndefinedBehaviorSanitizer)|UndefinedBehaviorSanitizer|runtime error:' \
        "$log"
}

compile_and_run() {
    local label=$1
    local sanitizer=$2
    local binary="$TMP_DIR/target/fel-543-$label"
    local compile_log="$TMP_DIR/logs/fel-543-$label-compile.log"
    local run_log="$TMP_DIR/logs/fel-543-$label-run.log"
    local compile_status=0
    local run_status=0
    local compile_args=(
        compile "$FIXTURE" "$binary" --fast --no-cache
    )

    if [ -n "$sanitizer" ]; then
        compile_args+=(--sanitize "$sanitizer")
    fi

    timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" "${compile_args[@]}" \
        >"$compile_log" 2>&1 || compile_status=$?
    if [ "$compile_status" -ne 0 ]; then
        failure_tail "FEL-543 $label compile" "$compile_log" \
            "$compile_status"
        return "$compile_status"
    fi

    if [ "$sanitizer" = "address" ]; then
        timeout --foreground --kill-after=5s 30s \
            bash "$ASAN_RUNNER" --target-root "$TMP_DIR/target" \
            --compile-log "$compile_log" -- "$binary" \
            >"$run_log" 2>&1 || run_status=$?
    else
        timeout --foreground --kill-after=5s 30s \
            env UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
                "$binary" >"$run_log" 2>&1 || run_status=$?
    fi
    if [ -n "$sanitizer" ] && sanitizer_reported_failure "$run_log"; then
        if [ "$run_status" -eq 0 ]; then
            run_status=1
        fi
        failure_tail "FEL-543 $label sanitizer" "$run_log" \
            "$run_status"
        return 1
    fi
    if [ "$run_status" -ne 0 ]; then
        failure_tail "FEL-543 $label executable" "$run_log" "$run_status"
        return "$run_status"
    fi
    return 0
}

[ -f "$FIXTURE" ] && [ ! -L "$FIXTURE" ] || {
    echo "FAIL: FEL-543 tracked regression fixture is unavailable" >&2
    exit 1
}
mkdir -p -- "$TMP_DIR/target" "$TMP_DIR/logs"

compile_and_run baseline ""
compile_and_run sanitize-undefined undefined
compile_and_run sanitize-address address

echo "PASS: FEL-543 safetensors class/Array return ownership matrix"
