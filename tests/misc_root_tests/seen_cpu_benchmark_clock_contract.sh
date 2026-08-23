#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"

fail() {
    echo "FAIL: CPU benchmark clock contract: $*" >&2
    exit 1
}

count=0
for benchmark in "$ROOT_DIR"/scripts/benchmark_*.py; do
    [ -f "$benchmark" ] && [ ! -L "$benchmark" ] ||
        fail "unsafe benchmark path: ${benchmark#"$ROOT_DIR"/}"
    count=$((count + 1))
    grep -Fq 'time.thread_time_ns()' "$benchmark" ||
        fail "benchmark does not use current-thread CPU time: ${benchmark#"$ROOT_DIR"/}"
    if grep -Fq 'time.perf_counter_ns()' "$benchmark"; then
        fail "benchmark charges runner descheduling: ${benchmark#"$ROOT_DIR"/}"
    fi
    if grep -Fq 'baseline_ratio_ppm' "$benchmark"; then
        [ "$(grep -Fc 'paired_median_ratio_ppm' "$benchmark")" -ge 2 ] ||
            fail "normalized benchmark does not preserve sample pairs: ${benchmark#"$ROOT_DIR"/}"
    fi
done

[ "$count" -eq 24 ] || fail "expected 24 benchmark scripts, found $count"

echo "PASS: CPU microbenchmarks exclude runner descheduling"
