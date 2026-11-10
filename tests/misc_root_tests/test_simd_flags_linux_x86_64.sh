#!/usr/bin/env bash
# Native Linux x86-64 CLI regression; no hardware or vectorization claim from reports.
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT/compiler_seen/target/seen}"
CAPPED="$ROOT/scripts/run_capped_regression.sh"
SCOPE=simd-flags-cli
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/simd-flags-cli.XXXXXX")"
timeout --foreground --kill-after=5s 60s clang -std=c11 -O0 -march=x86-64 \
    "$ROOT/tests/fixtures/test-002a/simd-cli/native_cpu_probe.c" \
    -o "$WORK/cpu-probe" >"$WORK/cpu-probe.compile.log" 2>&1
timeout --foreground --kill-after=5s 30s "$WORK/cpu-probe" >"$WORK/cpu-probe.run.log" 2>&1
cat "$WORK/cpu-probe.run.log"

SOURCE="$ROOT/tests/fixtures/test-002a/simd-cli/main.seen"
compile_deterministic() {
    timeout --foreground --kill-after=5s 600s \
        env SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
        SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
        bash "$SEEN_ATTESTED_COMPILER_RUNNER" --deterministic-environment \
        "$COMPILER" compile "$@" --no-cache --no-fork
}
compile_and_run() {
    local mode=$1
    shift
    if [ "$mode" = deterministic ]; then
        compile_deterministic \
            "$ROOT/tests/fixtures/test-002a/simd-cli/deterministic.seen" \
            "$WORK/$mode" "$@" \
            >"$WORK/$mode.compile.log" 2>&1
    else
        timeout --foreground --kill-after=5s 600s \
            bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" compile \
            "$SOURCE" "$WORK/$mode" --no-cache --no-fork "$@" \
            >"$WORK/$mode.compile.log" 2>&1
    fi
    timeout --foreground --kill-after=5s 30s "$WORK/$mode" \
        >"$WORK/$mode.run.log" 2>&1
    if [ "$mode" = deterministic ]; then
        [ ! -s "$WORK/$mode.run.log" ]
    else
        grep -Fxq 'Hello from SIMD test' "$WORK/$mode.run.log"
    fi
    echo "PASS: SIMD CLI $mode compilation and execution"
}
compile_and_run default
compile_and_run native --target-cpu=native
compile_and_run scalar --simd=none
bash "$ROOT/scripts/check_x86_executable_baseline.sh" x86-64 "$WORK/scalar"
compile_and_run deterministic --deterministic
bash "$ROOT/scripts/check_x86_executable_baseline.sh" x86-64 "$WORK/deterministic"
status=0
compile_deterministic "$SOURCE" "$WORK/deterministic-io" --deterministic \
    >"$WORK/deterministic-io.compile.log" 2>&1 || status=$?
[ "$status" -eq 1 ]
[ ! -e "$WORK/deterministic-io" ]
grep -Fq 'core.004d.nondeterministic-effect' "$WORK/deterministic-io.compile.log"
if grep -Fq 'Build succeeded' "$WORK/deterministic-io.compile.log"; then
    echo 'FAIL: deterministic I/O rejection reported build success' >&2
    exit 1
fi
compile_and_run avx2 --simd=avx2
compile_and_run v3 --target-cpu=x86-64-v3
compile_and_run report --simd-report
grep '^{"contract":"seen-simd-summary-v1"' "$WORK/report.compile.log" \
    >"$WORK/report.jsonl"
jq -es 'length >= 2 and all(.[]; .hardware_verified == false and .truncated == false)
    and any(.[]; .phase == "frontend") and any(.[]; .phase == "llvm-module")' \
    "$WORK/report.jsonl" >/dev/null
echo 'PASS: SIMD CLI flags, scalar baseline and machine-readable report'
