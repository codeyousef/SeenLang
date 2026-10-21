#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-inf-simd-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/inf-simd-b.XXXXXX")"
source "$ROOT_DIR/scripts/report_fixture_failure_logs.sh"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in "$SEEN_ARTIFACT_ROOT"/inf-simd-b.*) rm -rf -- "$TMP_DIR" ;; *) status=1 ;; esac
    else
        echo "Preserved x86 SIMD artifacts: $TMP_DIR" >&2
        seen_report_fixture_failure_logs "$TMP_DIR"
    fi
    exit "$status"
}
trap cleanup EXIT
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
compile_case() {
    local profile=$1 name=$2 source=$3
    shift 3
    local flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    if [ "$profile" = undefined ] || [ "$profile" = address ]; then flags+=("--sanitize=$profile"); fi
    local log="$TMP_DIR/$name-$profile.log"
    run_compiler compile "$source" "$TMP_DIR/$name-$profile" --no-cache --frozen \
        --target-cpu=x86-64 --emit-module-ir-dir "$TMP_DIR/$name-$profile-ir" "${flags[@]}" >"$log" 2>&1
    if [ "$profile" = address ]; then
        timeout --foreground --kill-after=5s 120s bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --target-root "$TMP_DIR" --compile-log "$log" -- "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1
    else
        UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
            timeout --foreground --kill-after=5s 120s "$TMP_DIR/$name-$profile" "$@" >>"$log" 2>&1
    fi
}
for profile in fast release; do
    compile_case "$profile" generic-fields "$ROOT_DIR/compiler_seen/tests/generic_field_type.seen"
done
compile_case fast trait-registry "$ROOT_DIR/tests/compiler_regressions/trait_registry_identity.seen"
rg -Fq 'PASS: trait registration identity deduplicates scans and preserves declared slots' "$TMP_DIR/trait-registry-fast.log"
for profile in fast release undefined address; do
    compile_case "$profile" trait-scalars "$ROOT_DIR/tests/compiler_regressions/dyn_fixed_width_returns.seen"
    compile_case "$profile" pointer-index "$ROOT_DIR/tests/compiler_regressions/typed_pointer_index.seen"
    compile_case "$profile" contract "$ROOT_DIR/tests/inference_simd/x86_dot_contract.seen"
    for name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: SEEN-INF-002B_$name" "$TMP_DIR/contract-$profile.log"
    done
    rg -q 'call <16 x i32> @llvm.x86.avx512.vpdpbusd.512' "$TMP_DIR/contract-$profile-ir"/*.ll
    rg -q 'call <16 x float> @llvm.x86.avx512bf16.dpbf16ps.512' "$TMP_DIR/contract-$profile-ir"/*.ll
    rg -q 'bitcast <16 x i32> .* to <32 x bfloat>' "$TMP_DIR/contract-$profile-ir"/*.ll
    rg -q 'noinline.*"target-features"=".*\+avx2' "$TMP_DIR/contract-$profile-ir"/*.ll
done
for name in bad_x86_target bad_x86_dot; do
    diagnostic=E_SIMD_TARGET
    if [ "$name" = bad_x86_dot ]; then diagnostic=E_SIMD_LANE; fi
    status=0
    run_compiler check "$ROOT_DIR/tests/inference_simd/$name.seen" >"$TMP_DIR/$name.log" 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then echo "FAIL: malformed x86 source accepted: $name" >&2; exit 1; fi
    case "$status" in 124|125|126|137|143) exit "$status" ;; esac
    rg -Fq "$diagnostic" "$TMP_DIR/$name.log"
done
clang -std=c11 -O2 -pthread -I "$ROOT_DIR/seen_runtime" \
    "$ROOT_DIR/tests/compiler_regressions/x86_os_state_contract.c" -lm -ldl \
    -o "$TMP_DIR/os-state" >"$TMP_DIR/os-state.log" 2>&1
timeout --foreground --kill-after=5s 60s "$TMP_DIR/os-state" >>"$TMP_DIR/os-state.log" 2>&1
benchmark_args=()
if [ "${SEEN_REQUIRE_X86_PERF:-0}" = 1 ]; then benchmark_args+=(--enforce); fi
compile_case release benchmark "$ROOT_DIR/tests/inference_simd/x86_dot_benchmark.seen" "${benchmark_args[@]}"
cat "$TMP_DIR/benchmark-release.log" | rg '^(BENCH:|PASS:|PENDING:)'
echo "PASS: checked x86 dispatch, packed lowering, scalar parity, OS state and lifetime contracts"
