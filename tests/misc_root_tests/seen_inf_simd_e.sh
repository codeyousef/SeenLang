#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-inf-simd-e
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/inf-simd-e.XXXXXX")"
SEEN_SIMD_ARTIFACT_PREFIX=inf-simd-e
source "$ROOT_DIR/scripts/simd_regression_support.sh"
SIMD_COMPILE_FLAGS=(--bounds-check --simd-report=full)
for profile in fast release undefined address; do
    compile_case "$profile" proof "$ROOT_DIR/tests/inference_simd/bounds_proof_contract.seen"
    for name in happy invalid limit reports cleanup; do
        rg -Fq "PASS: SEEN-INF-002E_$name" "$TMP_DIR/proof-$profile.log"
    done
    compile_case "$profile" tensor "$ROOT_DIR/tests/inference_simd/tensor_bounds_loops.seen"
    rg -Fq 'PASS: proved tensor bounds and conservative loop execution' "$TMP_DIR/tensor-$profile.log"
done
compile_case fast metadata-fuzz "$ROOT_DIR/tests/inference_simd/bounds_proof_fuzz.seen"
rg -Fq 'PASS: bounds proof AST and report metadata fuzz 60s seed=1101' "$TMP_DIR/metadata-fuzz-fast.log"
for profile in fast release; do
    flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    for fixture in unproved_shift_trap mutated_iteration_trap; do
        binary="$TMP_DIR/$fixture-$profile"
        run_compiler compile "$ROOT_DIR/tests/inference_simd/$fixture.seen" "$binary" \
            --no-cache --frozen --bounds-check --target-cpu=x86-64 "${flags[@]}" >"$TMP_DIR/00-active.log" 2>&1
        status=0
        timeout --foreground --kill-after=5s 30s "$binary" >>"$TMP_DIR/00-active.log" 2>&1 || status=$?
        [ "$status" -ne 0 ] || { echo "FAIL: unproved bounds access survived: $fixture" >&2; exit 1; }
        case "$status" in 124|125|126|137|143) exit "$status" ;; esac
        rg -Fq 'PANIC: index out of bounds' "$TMP_DIR/00-active.log"
    done
done
python3 "$ROOT_DIR/scripts/check_tensor_bounds_evidence.py" "$TMP_DIR/tensor-release-ir" "$TMP_DIR/tensor-release.log"
# The brief form must still report actual observations and a policy rejection.
run_compiler compile "$ROOT_DIR/tests/inference_simd/tensor_bounds_loops.seen" "$TMP_DIR/no-vector" \
    --no-cache --frozen --fast --bounds-check --simd=none --simd-report=brief --target-cpu=x86-64 \
    >"$TMP_DIR/00-active.log" 2>&1
rg -Fq 'disabled-by-policy' "$TMP_DIR/00-active.log"
rg -Fq 'seen-simd-summary-v1' "$TMP_DIR/00-active.log"
echo 'PASS: proved bounds, retained adverse guards, scoped identities and observed SIMD reports'
