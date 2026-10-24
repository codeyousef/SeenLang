#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-inf-simd-c
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/inf-simd-c.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=inf-simd-c
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" contract "$ROOT_DIR/tests/inference_simd/arm_dot_contract.seen"
    for name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: SEEN-INF-002C_$name" "$TMP_DIR/contract-$profile.log"
    done
done
rg '^\{' "$TMP_DIR/contract-release.log"
compile_case fast metadata-fuzz "$ROOT_DIR/tests/inference_simd/arm_capability_fuzz.seen"
rg -Fq 'PASS: ARM capability metadata fuzz 60s seed=1101' "$TMP_DIR/metadata-fuzz-fast.log"
compile_case fast benchmark-metadata "$ROOT_DIR/tests/inference_simd/dot_benchmark_metadata.seen"
rg -Fxq 'BENCH: selected_variant=1' "$TMP_DIR/benchmark-metadata-fast.log"
[ "$(rg -c '^BENCH: candidate_ns=' "$TMP_DIR/benchmark-metadata-fast.log")" = 30 ]
[ "$(rg -c '^BENCH: scalar_ns=' "$TMP_DIR/benchmark-metadata-fast.log")" = 30 ]
status=0
run_compiler check "$ROOT_DIR/tests/inference_simd/bad_arm_arity.seen" >"$TMP_DIR/00-active.log" 2>&1 || status=$?
if [ "$status" -eq 0 ]; then echo 'FAIL: malformed ARM intrinsic accepted' >&2; exit 1; fi
case "$status" in 124|125|126|137|143) exit "$status" ;; esac
rg -Fq E_SIMD_ARITY "$TMP_DIR/00-active.log"
clang -std=c11 -O2 -Wall -Wextra -Werror \
    "$ROOT_DIR/tests/compiler_regressions/arm_vector_state_contract.c" \
    "$ROOT_DIR/seen_runtime/src/arm_vector_abi.c" -Wl,--wrap=prctl \
    -o "$TMP_DIR/vector-state" >"$TMP_DIR/00-active.log" 2>&1
timeout --foreground --kill-after=5s 60s "$TMP_DIR/vector-state" >>"$TMP_DIR/00-active.log" 2>&1
for target in linux-arm64 macos-arm64 windows-x86_64 macos-x86_64; do
    ir_dir="$TMP_DIR/cross-$target"
    run_compiler compile "$ROOT_DIR/tests/inference_simd/arm_dot_contract.seen" "$TMP_DIR/cross-$target-unused" \
        --no-cache --frozen --fast --target="$target" --emit-module-ir-dir "$ir_dir" --stop-after-ir \
        >"$TMP_DIR/00-active.log" 2>&1
    case "$target" in
        linux-arm64) triple=aarch64-unknown-linux-gnu ;;
        macos-arm64) triple=arm64-apple-macosx14 ;;
        windows-x86_64) triple=x86_64-w64-windows-gnu ;;
        macos-x86_64) triple=x86_64-apple-macosx14 ;;
    esac
    sources=("$ir_dir"/*.ll)
    [ "${#sources[@]}" -gt 0 ] && [ "${#sources[@]}" -le 128 ]
    for ir in "${sources[@]}"; do
        [ -f "$ir" ] && [ ! -L "$ir" ]
        [ "$(wc -c <"$ir")" -le "$((16 * 1024 * 1024))" ]
        opt -passes=verify -disable-output "$ir" >>"$TMP_DIR/00-active.log" 2>&1
        clang -target "$triple" -c "$ir" -o "$ir.o" >>"$TMP_DIR/00-active.log" 2>&1
    done
    if [ "$target" = linux-arm64 ] || [ "$target" = macos-arm64 ]; then
        rg -q 'llvm.vscale.i64' "$ir_dir"/*.ll
        rg -q 'llvm.masked.load.nxv4f32.p0' "$ir_dir"/*.ll
        rg -Fq '"target-features"="+neon,+sve"' "$ir_dir"/*.ll
    fi
    printf '{"target":"%s","classification":"compile-only","hardware_certified":false}\n' "$target"
done
benchmark_args=()
if [ "${SEEN_REQUIRE_ARM_PERF:-0}" = 1 ]; then benchmark_args+=(--enforce); fi
compile_case release benchmark "$ROOT_DIR/tests/inference_simd/arm_dot_benchmark.seen" "${benchmark_args[@]}"
rg '^(BENCH:|PASS:|PENDING:)' "$TMP_DIR/benchmark-release.log"
echo 'PASS: ARM capability, scalar/native parity, ownership and cross-target IR/object contracts'
