#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-cuda-event-query
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/cuda-event-query.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=cuda-event-query
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fixture="$ROOT_DIR/tests/production_gpu_runtime"
timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror \
    -I "$ROOT_DIR/seen_runtime/cuda/include" "$fixture/cuda_event_query_header.c" -o "$TMP_DIR/header"
timeout --foreground --kill-after=5s 60s "$TMP_DIR/header"
if readelf -d "$TMP_DIR/header" | rg -i 'NEEDED.*(cuda|cublas)'; then
    echo 'FAIL: CPU-only header contract linked a GPU SDK' >&2; exit 1
fi
run_compiler check "$fixture/cuda_event_query/src/main.seen" > "$TMP_DIR/frontend.log" 2>&1
if [ "${SEEN_CUDA_QUERY_HARDWARE:-0}" != 1 ]; then
    echo 'PASS: CPU-only fixed-width header and Seen CUDA poll frontend; hardware not requested'
    exit 0
fi
# No software device or compatibility fallback is allowed in this branch.
export CUDACXX=/opt/cuda/bin/nvcc
timeout --foreground --kill-after=5s 300s cmake -S "$ROOT_DIR/seen_runtime/cuda" -B "$TMP_DIR/build" \
    -DSEEN_ENABLE_CUDA=ON -DCMAKE_BUILD_TYPE=RelWithDebInfo -DCMAKE_CUDA_ARCHITECTURES=89 > "$TMP_DIR/cmake.log" 2>&1
timeout --foreground --kill-after=5s 300s cmake --build "$TMP_DIR/build" --parallel 1 >> "$TMP_DIR/cmake.log" 2>&1
timeout --foreground --kill-after=5s 120s /opt/cuda/bin/nvcc -std=c++17 -arch=sm_89 --threads 1 -O2 \
    -I "$ROOT_DIR/seen_runtime/cuda/include" "$fixture/cuda_event_query_contract.cu" \
    -L "$TMP_DIR/build" -Xlinker=-rpath -Xlinker="$TMP_DIR/build" -lseen_cuda -o "$TMP_DIR/native"
timeout --foreground --kill-after=5s 60s clang -std=c11 -Wall -Wextra -Werror -fPIC -shared \
    -I /opt/cuda/include "$fixture/cuda_event_query_fault.c" -ldl -o "$TMP_DIR/query-fault.so"
for mode in reference pending error; do
    SEEN_CUDA_QUERY_FAULT="$mode" LD_PRELOAD="$TMP_DIR/query-fault.so" \
        timeout --foreground --kill-after=5s 120s "$TMP_DIR/native" "$mode" > "$TMP_DIR/native-$mode.log" 2>&1
    rg -Fq 'PASS: CUDA nonblocking query' "$TMP_DIR/native-$mode.log"
    rg -Fq 'waits=0' "$TMP_DIR/native-$mode.log"
done
timeout --foreground --kill-after=5s 180s /opt/cuda/bin/compute-sanitizer --tool memcheck \
    --leak-check full --error-exitcode 99 "$TMP_DIR/native" reference > "$TMP_DIR/compute-sanitizer.log" 2>&1
rg -Fq 'ERROR SUMMARY: 0 errors' "$TMP_DIR/compute-sanitizer.log"
for profile in fast release; do
    # Compile from the project containing the explicit bundled dependency.
    (cd "$fixture/cuda_event_query"; compile_fixture "$profile" query src/main.seen)
    for mode in reference pending error; do
        SEEN_CUDA_QUERY_FAULT="$mode" LD_PRELOAD="$TMP_DIR/query-fault.so" run_fixture "$profile" query "$mode"
        cp -- "$TMP_DIR/query-$profile.log" "$TMP_DIR/query-$profile-$mode.log"
        cp -- "$TMP_DIR/query-$profile.run.status" "$TMP_DIR/query-$profile-$mode.status"
        rg -Fq 'PASS: Seen CUDA event poll' "$TMP_DIR/query-$profile-$mode.log"
        rg -Fq 'waits=0' "$TMP_DIR/query-$profile-$mode.log"
    done
done
echo 'PASS: real RTX 4090 event query pending error pinned transfers Seen ownership and device memcheck'
