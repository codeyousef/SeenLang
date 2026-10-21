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
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/inf-simd-b.XXXXXX")"
SEEN_SIMD_ARTIFACT_PREFIX=inf-simd-b
source "$ROOT_DIR/scripts/simd_regression_support.sh"
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
