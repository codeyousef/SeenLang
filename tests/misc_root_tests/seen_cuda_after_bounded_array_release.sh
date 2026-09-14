#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] ||
   [ "${SEEN_JOBS:-0}" != 1 ] || [ "${SEEN_OPT_JOBS:-0}" != 1 ]; then
    echo "FAIL: FEL-1581 regression requires verified serial hard scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
TEST_ROOT="${SEEN_ARTIFACT_ROOT:?}/fel-1581"
mkdir -p "$TEST_ROOT"
cp -R "$ROOT_DIR/tests/fixtures/fel-1581" "$TEST_ROOT/project"

run_case() {
    local mode=$1
    local output="$TEST_ROOT/fel-1581-$mode"
    local ir_dir="$TEST_ROOT/ir-$mode"
    shift
    (
        cd "$TEST_ROOT/project"
        "$SEEN_BIN" compile src/main.seen "$output" "$@" \
            --target-cpu=x86-64 --no-cache --jobs 1 --opt-jobs 1 --no-fork \
            --emit-module-ir-dir "$ir_dir"
    )

    grep -REq 'call void @seen_arr_free\(ptr ' "$ir_dir" || {
        echo "FAIL: Array.free did not use the budget-aware runtime release" >&2
        exit 1
    }
    grep -REq 'define hidden .*@remove\(' "$ir_dir" || {
        echo "FAIL: env.remove was not hidden from native symbol interposition" >&2
        exit 1
    }

    # Non-exported Seen functions must not satisfy same-named native ABI
    # references. libcuda calls libc remove(3); a defined dynamic `remove`
    # here would reinterpret char* as SeenString and corrupt the allocator.
    if readelf --dyn-syms --wide "$output" |
        awk '$7 != "UND" && $8 == "remove" { found = 1 } END { exit !found }'; then
        echo "FAIL: non-exported Seen remove function escaped into the dynamic ABI" >&2
        exit 1
    fi
    if [ "${SEEN_FEL_1581_REQUIRE_CUDA:-0}" = 1 ]; then
        "$output"
    fi
}

run_case fast
run_case release --release --lto=thin

if [ "${SEEN_FEL_1581_REQUIRE_CUDA:-0}" != 1 ]; then
    echo "PASS: FEL-1581 compile-only (set SEEN_FEL_1581_REQUIRE_CUDA=1 for hardware execution)"
else
    echo "PASS: bounded array accounting and CUDA initialization in fast and release modes"
fi
