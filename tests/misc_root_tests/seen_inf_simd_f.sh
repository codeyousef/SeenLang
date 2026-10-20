#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-inf-simd-f
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/inf-simd-f.XXXXXX")"
SEEN_SIMD_ARTIFACT_PREFIX=inf-simd-f
SIMD_RUN_SECONDS=900
source "$ROOT_DIR/scripts/simd_regression_support.sh"
SIMD_COMPILE_FLAGS=(--bounds-check --simd-report=full)
for profile in fast release undefined address; do
    compile_case "$profile" quantized "$ROOT_DIR/tests/inference_simd/quantized_corpus.seen"
    rg -Fq 'PASS: quantized Q8_0 Q4_0 Q4_K Q2_K IQ3_XXS MXFP4 E4M3FN tails=all alignments=64 widths=scalar,128,256,512' "$TMP_DIR/quantized-$profile.log"
    rg -Fq 'PASS: quantized invalid geometry alias metadata cancellation deadline and allocation stability' "$TMP_DIR/quantized-$profile.log"
done
compile_case fast metadata-fuzz "$ROOT_DIR/tests/inference_simd/quantized_metadata_fuzz.seen"
rg -Fq 'PASS: quantized metadata fuzz 60s seed=1101 live-allocation-stable' "$TMP_DIR/metadata-fuzz-fast.log"
for width in 4 8 16; do
    rg -Fq "llvm.masked.store.v${width}f32.p0" "$TMP_DIR/quantized-release-ir"/*.ll
done
benchmark_args=()
if [ "${SEEN_REQUIRE_QUANTIZED_SIMD_PERF:-0}" = 1 ]; then benchmark_args+=(--enforce); fi
compile_case release benchmark "$ROOT_DIR/tests/inference_simd/quantized_benchmark.seen" "${benchmark_args[@]}"
rg '^(BENCH:|PASS:|PENDING:)' "$TMP_DIR/benchmark-release.log"
