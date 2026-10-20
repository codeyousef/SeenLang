#!/usr/bin/env bash
# Shared compile/run/cleanup policy for the SIMD regression entry points.
[ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" = 1 ] || {
    echo "RESOURCE STOP: SIMD helper requires an attested capped regression" >&2
    exit 126
}
: "${SEEN_SIMD_ARTIFACT_PREFIX:?}" "${ATTESTED_SEEN:?}" "${TMP_DIR:?}"
ulimit -c 0
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in "$SEEN_ARTIFACT_ROOT"/"$SEEN_SIMD_ARTIFACT_PREFIX".*) rm -rf -- "$TMP_DIR" ;; *) status=1 ;; esac
    else
        echo "Preserved SIMD artifacts: $TMP_DIR" >&2
        seen_report_fixture_failure_logs "$TMP_DIR"
    fi
    exit "$status"
}
trap cleanup EXIT
compile_case() {
    local profile=$1 name=$2 source=$3
    shift 3
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    if [ "$profile" = undefined ] || [ "$profile" = address ]; then flags+=("--sanitize=$profile"); fi
    local log="$TMP_DIR/00-active.log"
    run_compiler compile "$source" "$TMP_DIR/$name-$profile" --no-cache --frozen \
        --target-cpu=x86-64 --emit-module-ir-dir "$TMP_DIR/$name-$profile-ir" "${flags[@]}" >"$log" 2>&1
    if [ "$profile" = address ]; then
        timeout --foreground --kill-after=5s 120s bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --target-root "$TMP_DIR" --compile-log "$log" -- "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1
    else
        UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
            timeout --foreground --kill-after=5s 120s "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1
    fi
    cp -- "$log" "$TMP_DIR/$name-$profile.log"
}
