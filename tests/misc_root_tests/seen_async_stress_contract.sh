#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-stress
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/async-stress.XXXXXX")"
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/async-stress.*) rm -rf -- "$TMP_DIR" ;;
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
    shift 3
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    if [ "$profile" = undefined ] || [ "$profile" = thread ] || [ "$profile" = address ]; then
        flags+=("--sanitize=$profile")
    fi
    local log="$TMP_DIR/$name-$profile.log"
    bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$source" "$TMP_DIR/$name-$profile" \
        --no-cache --frozen --target-cpu=x86-64 \
        "${flags[@]}" >"$log" 2>&1
    if [ "$profile" = address ] || [ "$profile" = thread ]; then
        timeout --foreground --kill-after=5s 120s \
            bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --sanitizer "$profile" --target-root "$TMP_DIR" --compile-log "$log" -- \
            "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1
    else
        UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
            timeout --foreground --kill-after=5s 120s "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1
    fi
}

for profile in fast release undefined address thread; do
    compile_case "$profile" lifecycle "$ROOT_DIR/seen_std/tests/async/async-002g.seen"
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: ASYNC-002G_$case_name" "$TMP_DIR/lifecycle-$profile.log"
    done
    compile_case "$profile" io "$ROOT_DIR/seen_std/tests/async/io_completion_stress_fixture.seen" \
        "$TMP_DIR/completion-$profile.bin"
    for case_name in readiness_cancel completion_cancel cleanup; do
        rg -Fq "PASS: ASYNC-002G_io_$case_name" "$TMP_DIR/io-$profile.log"
    done
    rg '^BENCH: async-002g ' "$TMP_DIR/lifecycle-$profile.log"
done

echo "PASS: async software stress and live-resource checks in fast/release/UBSan/ASan/TSan"
cat <<'JSON'
{"schema":"seen-async-software-stress-v1","software_profiles":["fast","release","undefined","address","thread"],"warmups":5,"samples":30,"certified":false,"performance_regression_verified":false,"deferred":["designated-performance-baseline","7950x3d-scaling","gpu-completion"]}
JSON
echo "PENDING: designated performance baseline, 7950X3D scaling and GPU completion certification"
