#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-inf-simd-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURES="$ROOT_DIR/tests/inference_simd"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/inf-simd-a.XXXXXX")"
cleanup() {
    local status=$?
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/inf-simd-a.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] && rm -rf -- "$TMP_DIR"
            else
                echo "Preserved portable SIMD artifacts: $TMP_DIR" >&2
            fi ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }

for fixture in portable_plan vector_values; do
    run_compiler check "$FIXTURES/$fixture.seen" >"$TMP_DIR/$fixture-check.log" 2>&1
    for profile in fast release; do
        output="$TMP_DIR/$fixture-$profile"
        log="$output.log"
        ir_dir="$output-ir"
        flags=(--no-cache --frozen --target-cpu=x86-64
            --emit-module-ir-dir "$ir_dir" --simd-report)
        if [ "$profile" = release ]; then flags+=(--release --lto=thin)
        else flags+=(--fast); fi
        run_compiler compile "$FIXTURES/$fixture.seen" "$output" "${flags[@]}" >"$log" 2>&1
        timeout --foreground --kill-after=5s 120s "$output" >>"$log" 2>&1
        grep -Fq 'PASS:' "$log"
        grep -Fq '"contract":"seen-portable-simd-ir-v1"' "$log"
        grep -Fq '"hardware_verified":false' "$log"
        if [ "$fixture" = vector_values ]; then
            grep -Eq 'fcmp ogt <16 x float>' "$ir_dir"/*.ll
            grep -Eq 'select <16 x i1>' "$ir_dir"/*.ll
            grep -Eq 'insertelement <8 x i1>' "$ir_dir"/*.ll
            grep -Eq 'llvm.vector.reduce.add.v16i32' "$ir_dir"/*.ll
            grep -Eq 'llvm.vector.reduce.fadd.v8f64' "$ir_dir"/*.ll
        fi
    done
done
for fixture in bad_arity bad_mask bad_lane; do
    case "$fixture" in
        bad_arity) diagnostic=E_SIMD_ARITY ;;
        bad_mask) diagnostic=E_SIMD_MASK ;;
        bad_lane) diagnostic=E_SIMD_LANE ;;
    esac
    status=0
    run_compiler check "$FIXTURES/$fixture.seen" >"$TMP_DIR/$fixture.log" 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then
        echo "FAIL: malformed SIMD fixture accepted: $fixture" >&2
        exit 1
    fi
    case "$status" in 124|125|126|137|143) exit "$status" ;; esac
    grep -Fq "$diagnostic" "$TMP_DIR/$fixture.log"
done

echo 'PASS: portable SIMD representation, native lowering and explicit dispatch plans'
