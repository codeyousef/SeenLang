#!/usr/bin/env bash
# Shared compile/run/cleanup policy for the compiled fixture entry points.
[ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" = 1 ] || {
    echo "RESOURCE STOP: compiled fixture helper requires an attested capped regression" >&2
    exit 126
}
: "${FIXTURE_ARTIFACT_PREFIX:?}" "${ATTESTED_SEEN:?}" "${TMP_DIR:?}"
ulimit -c 0
FIXTURE_COMPILE_FLAGS=()
: "${FIXTURE_RUN_SECONDS:=120}"
case "$FIXTURE_RUN_SECONDS" in ''|*[!0-9]*) echo 'invalid fixture run time limit' >&2; exit 126 ;; esac
[ "$FIXTURE_RUN_SECONDS" -ge 1 ] && [ "$FIXTURE_RUN_SECONDS" -le 900 ] || exit 126
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in "$SEEN_ARTIFACT_ROOT"/"$FIXTURE_ARTIFACT_PREFIX".*) rm -rf -- "$TMP_DIR" ;; *) status=1 ;; esac
    else
        echo "Preserved fixture artifacts: $TMP_DIR" >&2
        seen_report_fixture_failure_logs "$TMP_DIR"
    fi
    exit "$status"
}
trap cleanup EXIT
compile_fixture() {
    local profile=$1 name=$2 source=$3
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    if [ "$profile" = undefined ] || [ "$profile" = address ]; then flags+=("--sanitize=$profile"); fi
    local log="$TMP_DIR/00-active.log" status=0
    printf '126\n' > "$TMP_DIR/$name-$profile.compile.status" || return
    run_compiler compile "$source" "$TMP_DIR/$name-$profile" --no-cache --frozen \
        --target-cpu="${FIXTURE_TARGET_CPU:-x86-64}" --emit-module-ir-dir "$TMP_DIR/$name-$profile-ir" "${flags[@]}" "${FIXTURE_COMPILE_FLAGS[@]}" >"$log" 2>&1 || status=$?
    cp -- "$log" "$TMP_DIR/$name-$profile.compile.log" || return
    cp -- "$log" "$TMP_DIR/$name-$profile.log" || return
    printf '%s\n' "$status" > "$TMP_DIR/$name-$profile.compile.status" || return
    return "$status"
}
run_fixture() {
    local profile=$1 name=$2
    shift 2
    local log="$TMP_DIR/00-active.log" status=0 compiled_status
    cp -- "$TMP_DIR/$name-$profile.compile.log" "$log" || return
    IFS= read -r compiled_status < "$TMP_DIR/$name-$profile.compile.status" || return 126
    if [ "$compiled_status" != 0 ]; then
        if [[ "$compiled_status" =~ ^[1-9][0-9]{0,2}$ ]] && [ "$compiled_status" -le 255 ]; then return "$compiled_status"; fi
        echo 'FAIL: invalid fixture compile status' >&2; return 126
    fi
    printf '126\n' > "$TMP_DIR/$name-$profile.run.status" || return
    if [ "$profile" = address ]; then
        local preload_args=()
        if [ -n "${FIXTURE_ASAN_PRELOAD_LIBRARY:-}" ]; then
            preload_args=(--fixture-preload "$FIXTURE_ASAN_PRELOAD_LIBRARY"
                --fixture-preload-sha256 "$(sha256sum -- "$FIXTURE_ASAN_PRELOAD_LIBRARY" | cut -d ' ' -f1)")
        fi
        timeout --foreground --kill-after=5s "${FIXTURE_RUN_SECONDS}s" bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --target-root "$TMP_DIR" --compile-log "$log" "${preload_args[@]}" -- "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1 || status=$?
    else
        UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
            timeout --foreground --kill-after=5s "${FIXTURE_RUN_SECONDS}s" "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1 || status=$?
    fi
    cp -- "$log" "$TMP_DIR/$name-$profile.log" || return
    printf '%s\n' "$status" > "$TMP_DIR/$name-$profile.run.status" || return
    return "$status"
}
compile_case() {
    local profile=$1 name=$2 source=$3
    shift 3
    compile_fixture "$profile" "$name" "$source" || return
    run_fixture "$profile" "$name" "$@"
}
