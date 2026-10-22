#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-multithread-executor
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/multithread-executor.XXXXXX")"
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/multithread-executor.*) rm -rf -- "$TMP_DIR" ;;
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
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    local log="$TMP_DIR/$name-$profile.log"
    bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$source" "$TMP_DIR/$name-$profile" \
        --no-cache --frozen --target-cpu=x86-64 --emit-module-ir-dir "$TMP_DIR/$name-$profile-ir" \
        "${flags[@]}" >"$log" 2>&1
    timeout --foreground --kill-after=5s 120s "$TMP_DIR/$name-$profile" >>"$log" 2>&1
    test -d "$TMP_DIR/$name-$profile-ir"
    rg --files "$TMP_DIR/$name-$profile-ir" -g '*.ll' > "$TMP_DIR/$name-$profile-ir-files.log"
    test -s "$TMP_DIR/$name-$profile-ir-files.log"
    local dispatch_status=0
    rg -n '@"?T_(run|release|onEvent)"?\(' "$TMP_DIR/$name-$profile-ir" >>"$log" || dispatch_status=$?
    if [ "$dispatch_status" -ne 1 ]; then
        echo 'FAIL: unresolved generic callback dispatch or unreadable emitted LLVM' >&2
        exit 1
    fi
}

for profile in fast release; do
    compile_case "$profile" trait-registry "$ROOT_DIR/tests/compiler_regressions/trait_registry_identity.seen"
    compile_case "$profile" fixed-width-array "$ROOT_DIR/tests/compiler_regressions/fixed_width_array_fill.seen"
    compile_case "$profile" boolean-array "$ROOT_DIR/tests/compiler_regressions/boolean_array_literals.seen"
    compile_case "$profile" pointer-index "$ROOT_DIR/tests/compiler_regressions/typed_pointer_index.seen"
    compile_case "$profile" trait-scalars "$ROOT_DIR/tests/compiler_regressions/dyn_fixed_width_returns.seen"
    compile_case "$profile" trait-methods "$ROOT_DIR/tests/compiler_regressions/dyn_method_arguments.seen"
    rg -Fq 'PASS: static instance and forwarded trait arguments preserve identity and box ownership' "$TMP_DIR/trait-methods-$profile.log"
    compile_case "$profile" generic-array "$ROOT_DIR/tests/fixtures/async/generic_array_field_cleanup.seen"
    compile_case "$profile" generic-field-type "$ROOT_DIR/compiler_seen/tests/generic_field_type.seen"
    compile_case "$profile" generic-result-inference "$ROOT_DIR/tests/fixtures/async/generic_result_inference.seen"
    compile_case "$profile" generic-call-plan "$ROOT_DIR/compiler_seen/tests/generic_call_type.seen"
    compile_case "$profile" generic-send "$ROOT_DIR/tests/fixtures/async/generic_send_registry.seen"
    compile_case "$profile" generic-call-ownership "$ROOT_DIR/tests/fixtures/async/generic_call_ownership.seen"
    compile_case "$profile" pool-release "$ROOT_DIR/tests/fixtures/async/pool_release_pointer_abi.seen"
    compile_case "$profile" lifecycle "$ROOT_DIR/seen_std/tests/async/async-002a.seen"
    compile_case "$profile" imported "$ROOT_DIR/tests/fixtures/async/cross_module.seen"
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: ASYNC-002A_$case_name" "$TMP_DIR/lifecycle-$profile.log"
    done
done

check_rejection() {
    local name=$1 source=$2 expected=$3
    local log="$TMP_DIR/$name.log"
    if bash "$ATTESTED_SEEN" "$COMPILER" check "$source" >"$log" 2>&1; then
        echo "FAIL: executor accepted negative fixture $name" >&2
        exit 1
    fi
    rg -Fq "$expected" "$log"
    if rg -q 'error\[E_(TYPE|BINDING)_UNKNOWN\]' "$log"; then
        echo "FAIL: negative fixture $name contains unresolved types or bindings" >&2
        exit 1
    fi
}

check_rejection unsafe "$ROOT_DIR/tests/fixtures/async/unsafe_job.seen" E_TYPE_BOUND
check_rejection generic-move "$ROOT_DIR/tests/fixtures/async/generic_move_rejected.seen" E_OWNERSHIP_USE_AFTER_MOVE
check_rejection unsafe-owned "$ROOT_DIR/tests/fixtures/async/unsafe_owned_job.seen" E_TYPE_BOUND

echo "PASS: fixed-worker typed execution in fast and release/ThinLTO"
