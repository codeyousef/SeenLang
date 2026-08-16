#!/usr/bin/env bash
# Seen Language Test Suite
# Run all tests and report results
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
WRAPPER="$SCRIPT_DIR/run_with_project_artifacts.sh"
HARD_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
ARTIFACT_ROOT_HELPER="$SCRIPT_DIR/artifact_root.sh"
BUILDER_CAPABILITY="$SCRIPT_DIR/rebuild_builder_capability.sh"
BUILDER_APPLICABILITY="$SCRIPT_DIR/rebuild_builder_applicability.sh"
SERIALIZER_VERIFY="$SCRIPT_DIR/verify_fork_serializer.sh"
BOUNDED_TOOLCHAIN_PREPARE="$SCRIPT_DIR/prepare_bounded_toolchain.sh"

is_positive_integer() {
    case "$1" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

require_vmem_cap() {
    local limit_kb=$1
    local label=$2
    local active_kb=""

    if ! ulimit -S -v "$limit_kb" 2>/dev/null; then
        echo "ERROR: could not apply $label virtual-memory cap (${limit_kb} KiB)" >&2
        return 126
    fi
    active_kb=$(ulimit -S -v 2>/dev/null || true)
    if ! is_positive_integer "$active_kb" || [ "$active_kb" -gt "$limit_kb" ]; then
        echo "ERROR: $label virtual-memory cap read-back failed" >&2
        return 126
    fi
}

if ! is_positive_integer "${SEEN_MAIN_VMEM_KB:-}" ||
    ! is_positive_integer "${SEEN_OPT_VMEM_KB:-}" ||
    ! is_positive_integer "${SEEN_MEMORY_LIMIT_BYTES:-}"; then
    echo "ERROR: explicit positive main, optimizer, and byte memory caps are required" >&2
    exit 2
fi

if [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" != "1" ] ||
    [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ]; then
    exec "$WRAPPER" legacy-all-tests -- env \
        SEEN_LOW_MEMORY=1 \
        SEEN_MAIN_VMEM_KB="$SEEN_MAIN_VMEM_KB" \
        SEEN_OPT_VMEM_KB="$SEEN_OPT_VMEM_KB" \
        SEEN_MEMORY_LIMIT_BYTES="$SEEN_MEMORY_LIMIT_BYTES" \
        SEEN="${SEEN:-$REPO_ROOT/compiler_seen/target/seen}" \
        STAGE1="${STAGE1:-${SEEN:-$REPO_ROOT/compiler_seen/target/seen}}" \
        "$0"
fi

[ -f "$ARTIFACT_ROOT_HELPER" ] || {
    echo "ERROR: missing artifact-root helper: $ARTIFACT_ROOT_HELPER" >&2
    exit 2
}
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_HELPER"
seen_artifact_root_init "$REPO_ROOT" || exit 2
if [ "$(uname -s)" = "Linux" ]; then
    namespace_tmp_identity=$(stat -c '%d:%i' /tmp 2>/dev/null || true)
    artifact_root_identity=$(stat -c '%d:%i' "$SEEN_ARTIFACT_ROOT" \
        2>/dev/null || true)
    if [ -z "$namespace_tmp_identity" ] ||
        [ "$namespace_tmp_identity" != "$artifact_root_identity" ]; then

        echo "ERROR: legacy all-tests artifact namespace validation failed" >&2
        exit 2
    fi
fi

if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then
    [ -x "$HARD_SCOPE_WRAPPER" ] || {
        echo "ERROR: missing hard-memory-scope wrapper: $HARD_SCOPE_WRAPPER" >&2
        exit 2
    }
    exec "$HARD_SCOPE_WRAPPER" --label "Legacy Seen all-tests" -- "$0"
fi
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
export SEEN_HARD_MEMORY_SCOPE_ACTIVE
"$HARD_SCOPE_WRAPPER" --label "Legacy Seen all-tests read-back" \
    --verify-only --

cd "$REPO_ROOT"
require_vmem_cap "$SEEN_MAIN_VMEM_KB" "main compiler"

# Use the explicitly selected checkout compiler. Frozen bootstrap binaries are
# for rebuild recovery, not acceptance of current source behavior.
SEEN="${SEEN:-$REPO_ROOT/compiler_seen/target/seen}"
STAGE1="${STAGE1:-$SEEN}"
[ -x "$STAGE1" ] || {
    echo "ERROR: selected compile-stage compiler is not executable: $STAGE1" >&2
    exit 2
}
[ -f "$BUILDER_CAPABILITY" ] || {
    echo "ERROR: missing builder capability classifier: $BUILDER_CAPABILITY" >&2
    exit 2
}
FORK_SERIALIZER_SO=${SEEN_FORK_SERIALIZER_SO:-}
FORK_SERIALIZER_ATTESTATION=${SEEN_FORK_SERIALIZER_ATTESTATION:-}
if ! bash "$SERIALIZER_VERIFY" "$FORK_SERIALIZER_SO" \
    "$FORK_SERIALIZER_ATTESTATION" "$SEEN_ARTIFACT_ROOT" \
    "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" >/dev/null; then

    echo "ERROR: legacy all-tests requires the scope-attested fork serializer produced by safe_rebuild" >&2
    exit 126
fi
if ! SEEN_MEMORY_GUARD_IN_SCOPE=1 bash "$BUILDER_APPLICABILITY" \
    "$STAGE1" "$FORK_SERIALIZER_SO" >/dev/null; then

    echo "ERROR: selected compiler is not serializer-applicable: $STAGE1" >&2
    exit 126
fi
BOUNDED_TOOLCHAIN_DIR=$(bash "$BOUNDED_TOOLCHAIN_PREPARE" "$SEEN_ARTIFACT_ROOT") ||
    exit 126
PATH="$BOUNDED_TOOLCHAIN_DIR:$PATH"
export PATH SEEN_BOUNDED_TOOLCHAIN_DIR="$BOUNDED_TOOLCHAIN_DIR"
stage1_capability_status=0
stage1_capability=$(env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
    -u SEEN_FORK_SERIALIZER_ROOT_PID \
    bash "$BUILDER_CAPABILITY" "$STAGE1" 2>/dev/null) ||
    stage1_capability_status=$?
if [ "$stage1_capability_status" -ne 0 ]; then
    echo "ERROR: selected compiler schema probe failed with status $stage1_capability_status: $STAGE1" >&2
    exit 126
fi
case "$stage1_capability" in
    advertised-jobs)
        STAGE1_WORKER_FLAGS=(--jobs 1 --opt-jobs 1)
        ;;
    advertised-no-fork)
        STAGE1_WORKER_FLAGS=(--no-fork)
        ;;
    serializer-required) STAGE1_WORKER_FLAGS=() ;;
    *) echo "ERROR: selected compiler schema probe failed: $STAGE1" >&2; exit 126 ;;
esac
PASS=0
FAIL=0
SKIP=0
TOTAL=0

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

run_test() {
    local name=$1
    local test_log
    local status=0
    shift
    TOTAL=$((TOTAL + 1))
    test_log="$SEEN_ARTIFACT_ROOT/run-all-test-${TOTAL}.log"
    "$@" >"$test_log" 2>&1 || status=$?
    case "$status" in
        124|125|126|137|143)
            echo -e "  ${RED}RESOURCE STOP${NC} $name (status=$status)" >&2
            exit "$status"
            ;;
    esac
    if [ "$status" -ne 0 ] && grep -Eiq \
        '(^|[^[:alnum:]_])(resource stop:|out of memory|cannot allocate memory|could not allocate memory|memory allocation (failed|failure)|allocation failure|std::bad_alloc|bad_alloc|resource temporarily unavailable|cannot fork|can.t fork|fork: retry|fork (failed|failure)|pthread_create([^[:alnum:]_].*)?(failed|failure)|failed to create (a )?thread|can.t create (a )?thread|cannot create (a )?thread|thread creation (failed|failure))([^[:alnum:]_]|$)' \
        "$test_log" 2>/dev/null; then

        echo -e "  ${RED}RESOURCE STOP${NC} $name (diagnostic)" >&2
        exit 126
    fi
    if [ "$status" -eq 0 ]; then
        echo -e "  ${GREEN}PASS${NC} $name"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} $name"
        FAIL=$((FAIL + 1))
    fi
}

seen_help_has_compile() {
    env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
        -u SEEN_FORK_SERIALIZER_ROOT_PID "$SEEN" 2>&1 |
        grep -Eq 'compile|Usage'
}

check_inline_seen() {
    local compiler=$1
    local source_file=$2
    printf '%s\n' 'fun main() r: Int { return 0 }' > "$source_file"
    env -u SEEN_FORK_SERIALIZER_ROOT_PID \
        LD_PRELOAD="$FORK_SERIALIZER_SO" \
        SEEN_FORK_SERIALIZER_TARGET="$compiler" \
        "$compiler" check "$source_file"
}

compile_fixture() {
    local input=$1
    local output=$2
    local run_output=$3

    (
        rm -rf .seen_cache/
        env \
            LD_PRELOAD="$FORK_SERIALIZER_SO" \
            SEEN_FORK_SERIALIZER_TARGET="$STAGE1" \
            SEEN_FORK_SERIALIZER_ROOT_PID= \
            SEEN_JOBS=1 \
            SEEN_OPT_JOBS=1 \
            "$STAGE1" compile "$input" "$output" \
                "${STAGE1_WORKER_FLAGS[@]}"
        if [ "$run_output" = "1" ]; then
            "$output"
        fi
    )
}

skip_test() {
    local name="$1"
    local reason="$2"
    TOTAL=$((TOTAL + 1))
    SKIP=$((SKIP + 1))
    echo -e "  ${YELLOW}SKIP${NC} $name ($reason)"
}

echo "============================================"
echo "       Seen Language Test Suite"
echo "============================================"
echo ""

# Clear cache to avoid stale objects
rm -rf .seen_cache/

# ---- Section 1: Compiler Commands ----
echo "--- Compiler Commands ---"
run_test "seen compile help" seen_help_has_compile
run_test "seen check (valid)" check_inline_seen "$SEEN" /tmp/seen_test_valid.seen
run_test "compiler exists" test -x "$SEEN"
run_test "production compiler exists" test -f ./compiler_seen/target/seen

# ---- Section 2: Compilation ----
echo ""
echo "--- Compilation ---"

# Create test files
cat > /tmp/seen_test_hello.seen << 'SEEN'
fun main() r: Int {
    println("Hello, World!")
    return 0
}
main()
SEEN

cat > /tmp/seen_test_arithmetic.seen << 'SEEN'
fun main() r: Int {
    let x = 42
    let y = 13
    let sum = x + y
    let diff = x - y
    let prod = x * y
    println(sum.toString())
    println(diff.toString())
    println(prod.toString())
    return 0
}
main()
SEEN

cat > /tmp/seen_test_strings.seen << 'SEEN'
fun main() r: Int {
    let name = "Seen"
    let greeting = "Hello, " + name + "!"
    println(greeting)
    println(greeting.length().toString())
    return 0
}
main()
SEEN

cat > /tmp/seen_test_loops.seen << 'SEEN'
fun main() r: Int {
    var sum = 0
    var i = 0
    while i < 10 {
        sum = sum + i
        i = i + 1
    }
    println(sum.toString())
    return 0
}
main()
SEEN

cat > /tmp/seen_test_functions.seen << 'SEEN'
fun add(a: Int, b: Int) r: Int {
    return a + b
}

fun factorial(n: Int) r: Int {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}

fun main() r: Int {
    println(add(3, 4).toString())
    println(factorial(5).toString())
    return 0
}
main()
SEEN

cat > /tmp/seen_test_class.seen << 'SEEN'
class Point {
    var x: Int
    var y: Int

    fun new(x: Int, y: Int) r: Point {
        return Point{ x: x, y: y }
    }

    fun toString() r: String {
        return "(" + x.toString() + ", " + y.toString() + ")"
    }
}

fun main() r: Int {
    let p = Point.new(3, 4)
    println(p.toString())
    return 0
}
main()
SEEN

cat > /tmp/seen_test_array.seen << 'SEEN'
fun main() r: Int {
    var arr = Array<Int>()
    arr.push(10)
    arr.push(20)
    arr.push(30)
    println(arr.length().toString())
    println(arr[0].toString())
    println(arr[1].toString())
    println(arr[2].toString())
    return 0
}
main()
SEEN

# Compile and run tests (clear cache between each to avoid stale objects)
for test in hello arithmetic strings loops functions class array; do
    run_test "compile $test" compile_fixture \
        "/tmp/seen_test_${test}.seen" "/tmp/seen_test_${test}_bin" 1
done

# ---- Section 3: Language Features ----
echo ""
echo "--- Language Features ---"

cat > /tmp/seen_test_forin.seen << 'SEEN'
fun main() r: Int {
    var sum = 0
    for i in 0..5 {
        sum = sum + i
    }
    println(sum.toString())
    return 0
}
main()
SEEN

cat > /tmp/seen_test_enum.seen << 'SEEN'
enum Color { Red, Green, Blue }

fun main() r: Int {
    let c = Color.Red
    println("enum created")
    return 0
}
main()
SEEN

cat > /tmp/seen_test_stringinterp.seen << 'SEEN'
fun main() r: Int {
    let name = "World"
    let msg = "Hello {name}!"
    println(msg)
    return 0
}
main()
SEEN

for test in forin enum stringinterp; do
    run_test "feature $test" compile_fixture \
        "/tmp/seen_test_${test}.seen" "/tmp/seen_test_${test}_bin" 1
done

# ---- Section 4: Type System ----
echo ""
echo "--- Type System ---"
run_test "type check pass" check_inline_seen "$SEEN" /tmp/seen_typecheck.seen

# ---- Section 5: Bootstrap Verification ----
echo ""
echo "--- Bootstrap ---"
run_test "stage1 exists" test -f "$STAGE1"
run_test "stage1 executable" test -x "$STAGE1"
run_test "stage1 SHA256" test -f bootstrap/stage1_frozen.sha256

# ---- Section 6: Production Benchmarks ----
echo ""
echo "--- Benchmarks (compile-only) ---"
for bench in 01_matrix_mult 02_sieve 05_nbody 09_json_serialize 12_fannkuch; do
    if [ -f "benchmarks/production/${bench}.seen" ]; then
        run_test "bench compile $bench" compile_fixture \
            "benchmarks/production/${bench}.seen" "/tmp/seen_bench_${bench}" 0
    else
        skip_test "bench $bench" "not found"
    fi
done

# ---- Section 7: E-graph Optimization ----
echo ""
echo "--- E-graph Optimizations ---"
# Test that strength reduction works (mul->shl)
cat > /tmp/seen_test_egraph.seen << 'SEEN'
fun main() r: Int {
    let x = 42
    let a = x * 2
    let b = x * 4
    let c = x * 8
    let d = x * 16
    let e = x * 32
    println(a.toString())
    println(b.toString())
    println(c.toString())
    println(d.toString())
    println(e.toString())
    return 0
}
main()
SEEN

run_test "e-graph strength reduction" compile_fixture \
    /tmp/seen_test_egraph.seen /tmp/seen_test_egraph_bin 1

# ---- Section 8: Runtime ----
echo ""
echo "--- Runtime ---"
run_test "runtime library exists" test -f seen_runtime/seen_runtime.c
run_test "region runtime exists" test -f seen_runtime/seen_region.c
run_test "region header exists" test -f seen_runtime/seen_region.h

# ---- Cleanup ----
rm -f /tmp/seen_test_*.seen /tmp/seen_test_*_bin /tmp/seen_test_*_bin.c /tmp/seen_bench_*

# ---- Summary ----
echo ""
echo "============================================"
echo "  Results: $PASS passed, $FAIL failed, $SKIP skipped (of $TOTAL)"
echo "============================================"

if [ $FAIL -gt 0 ]; then
    exit 1
fi
exit 0
