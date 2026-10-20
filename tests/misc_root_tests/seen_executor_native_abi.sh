#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-executor-native-abi
if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" --platform "$SCOPE" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-platform-active "$SCOPE"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/executor-native.XXXXXX")"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$WORK_DIR" in
            "$SEEN_ARTIFACT_ROOT"/executor-native.*) rm -rf -- "$WORK_DIR" ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved executor artifacts: $WORK_DIR" >&2
    fi
    exit "$status"
}
trap cleanup EXIT

run_probe() {
    local name=$1
    local sanitizer=$2
    shift 2
    local flags=()
    if [ -n "$sanitizer" ]; then flags+=("-fsanitize=$sanitizer"); fi
    local label="${name}-${sanitizer:-native}"
    local log="$WORK_DIR/$label.compile.log"
    (
        set -x
        clang -std=c11 -O1 -g -pthread -ffunction-sections -fdata-sections \
            -I "$ROOT_DIR/seen_runtime" "${flags[@]}" "$@" \
            -Wl,--gc-sections -lm -ldl -o "$WORK_DIR/$label"
    ) >"$log" 2>&1
    if [ "$sanitizer" = address ]; then
        bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --target-root "$WORK_DIR" --compile-log "$log" -- "$WORK_DIR/$label"
    else
        timeout --foreground --kill-after=5s 120s "$WORK_DIR/$label"
    fi
}

for sanitizer in '' undefined address; do
    run_probe threads "$sanitizer" -Wall -Wextra -Werror \
        -DSEEN_EXECUTOR_THREAD_SLOTS=4 \
        "$ROOT_DIR/tests/compiler_regressions/executor_abi_contract.c" \
        "$ROOT_DIR/seen_runtime/src/executor_abi.c" -Wl,--wrap=pthread_create
    run_probe allocator "$sanitizer" \
        "$ROOT_DIR/tests/compiler_regressions/executor_allocator_contract.c" \
        "$ROOT_DIR/seen_runtime/seen_runtime.c"
done

# TSan needs a separate virtual-address policy on some hosts. Opt in on the
# certification host; a missing TSan run is not represented as a pass.
if [ "${SEEN_EXECUTOR_TSAN:-0}" = 1 ]; then
    run_probe allocator thread \
        "$ROOT_DIR/tests/compiler_regressions/executor_allocator_contract.c" \
        "$ROOT_DIR/seen_runtime/seen_runtime.c"
fi
echo "PASS: fixed-worker native resource contracts"
