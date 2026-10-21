#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-inf-simd-d
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/inf-simd-d.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=inf-simd-d
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" operations "$ROOT_DIR/tests/inference_simd/portable_operations.seen"
    for name in happy partial packing invalid cancel cleanup; do
        rg -Fq "PASS: SEEN-INF-002D_$name" "$TMP_DIR/operations-$profile.log"
    done
done
compile_case fast metadata-fuzz "$ROOT_DIR/tests/inference_simd/operation_metadata_fuzz.seen"
rg -Fq 'PASS: portable SIMD operation metadata fuzz 60s seed=1101' "$TMP_DIR/metadata-fuzz-fast.log"
for fixture in bad_operation_arity bad_operation_lane bad_operation_name bad_shuffle_index bad_shuffle_dynamic; do
    case "$fixture" in
        bad_operation_arity) expected=E_SIMD_ARITY ;;
        bad_operation_lane) expected=E_SIMD_LANE ;;
        bad_operation_name) expected=E_SIMD_OPERATION ;;
        bad_shuffle_index|bad_shuffle_dynamic) expected=E_SIMD_INDEX ;;
    esac
    status=0
    run_compiler check "$ROOT_DIR/tests/inference_simd/$fixture.seen" >"$TMP_DIR/00-active.log" 2>&1 || status=$?
    [ "$status" -ne 0 ] || { echo "FAIL: malformed SIMD operation accepted: $fixture" >&2; exit 1; }
    case "$status" in 124|125|126|137|143) exit "$status" ;; esac
    rg -Fq "$expected" "$TMP_DIR/00-active.log"
done
ir_dir="$TMP_DIR/operations-release-ir"
for pattern in llvm.masked.load.v4f32.p0 llvm.masked.load.v8f32.p0 llvm.masked.load.v16f32.p0 \
    llvm.masked.store.v16i8.p0 llvm.fma.v16f32 llvm.prefetch.p0 shufflevector; do
    rg -Fq "$pattern" "$ir_dir"/*.ll
done
rg -q '= zext <16 x i8>' "$ir_dir"/*.ll
rg -q '= sext <16 x i8>' "$ir_dir"/*.ll
rg -q '= trunc <16 x i32>' "$ir_dir"/*.ll
echo 'PASS: portable masked memory, widening, saturation, packing, permutes, reductions, FMA and hints'
benchmark_args=()
if [ "${SEEN_REQUIRE_SIMD_OPERATION_PERF:-0}" = 1 ]; then benchmark_args+=(--enforce); fi
compile_case release benchmark "$ROOT_DIR/tests/inference_simd/portable_operation_benchmark.seen" "${benchmark_args[@]}"
rg '^(BENCH:|PASS:|PENDING:)' "$TMP_DIR/benchmark-release.log"
