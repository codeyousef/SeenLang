#!/usr/bin/env bash
# Native Linux x86-64 CLI regression; no hardware or vectorization claim from reports.
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT/compiler_seen/target/seen}"
CAPPED="$ROOT/scripts/run_capped_regression.sh"
SCOPE=simd-integration-cli
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/simd-integration-cli.XXXXXX")"
timeout --foreground --kill-after=5s 60s clang -std=c11 -O0 -march=x86-64 \
    "$ROOT/tests/fixtures/test-002a/simd-cli/native_cpu_probe.c" \
    -o "$WORK/cpu-probe" >"$WORK/cpu-probe.compile.log" 2>&1
timeout --foreground --kill-after=5s 30s "$WORK/cpu-probe" >"$WORK/cpu-probe.run.log" 2>&1
cat "$WORK/cpu-probe.run.log"

SOURCE="$ROOT/tests/misc_root_tests/test_simd_codegen.seen"
compile_and_run() {
    local mode=$1
    shift
    timeout --foreground --kill-after=5s 600s \
        bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" compile \
        "$SOURCE" "$WORK/$mode" --no-cache --no-fork "$@" \
        >"$WORK/$mode.compile.log" 2>&1
    timeout --foreground --kill-after=5s 30s "$WORK/$mode" \
        >"$WORK/$mode.run.log" 2>&1
    grep -Fxq 'dot_product(1000) ~ 249625125' "$WORK/$mode.run.log"
    grep -Fxq 'array_sum(1000) ~ 1000000' "$WORK/$mode.run.log"
}
compile_and_run avx2 --simd=avx2 --simd-report=full
compile_and_run scalar --simd=none
compile_and_run auto --simd=auto
cmp -- "$WORK/avx2.run.log" "$WORK/scalar.run.log"
cmp -- "$WORK/auto.run.log" "$WORK/scalar.run.log"
bash "$ROOT/scripts/check_x86_executable_baseline.sh" x86-64 "$WORK/scalar"
grep '^{"contract":"seen-simd-summary-v1"' "$WORK/avx2.compile.log" \
    >"$WORK/report.jsonl"
jq -es 'length >= 2 and all(.[]; .hardware_verified == false and .truncated == false)
    and any(.[]; .phase == "frontend") and any(.[]; .phase == "llvm-module")' \
    "$WORK/report.jsonl" >/dev/null

for backend in mlir cranelift; do
    status=0
    timeout --foreground --kill-after=5s 60s \
        bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" compile \
        "$SOURCE" "$WORK/$backend" --no-cache --no-fork "--backend=$backend" \
        >"$WORK/$backend.compile.log" 2>&1 || status=$?
    [ "$status" -eq 1 ]
    [ ! -e "$WORK/$backend" ]
    grep -Fq "Backend '$backend' is not implemented in this binary. Supported backend: llvm." \
        "$WORK/$backend.compile.log"
    if grep -Fq 'Build succeeded' "$WORK/$backend.compile.log"; then
        echo 'FAIL: unsupported backend reported build success' >&2
        exit 1
    fi
done
echo 'PASS: SIMD exact scalar parity, structured report, baseline and backend rejection'
