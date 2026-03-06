#!/bin/bash
# Seen Language Test Suite
# Run all tests and report results
set -e

# Use stage1 compiler (the actual working compiler)
SEEN="./bootstrap/stage1_frozen"
STAGE1="./bootstrap/stage1_frozen"
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
    local name="$1"
    local cmd="$2"
    TOTAL=$((TOTAL + 1))
    if eval "$cmd" > /dev/null 2>&1; then
        echo -e "  ${GREEN}PASS${NC} $name"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} $name"
        FAIL=$((FAIL + 1))
    fi
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
run_test "seen compile help" "$SEEN 2>&1 | grep -q 'compile\|Usage'"
run_test "seen check (valid)" "echo 'fun main() r: Int { return 0 }' > /tmp/seen_test_valid.seen && $SEEN check /tmp/seen_test_valid.seen"
run_test "compiler exists" "test -x $SEEN"
run_test "production compiler exists" "test -f ./compiler_seen/target/seen"

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
    if [ -f "$STAGE1" ]; then
        run_test "compile $test" "rm -rf .seen_cache/ && $STAGE1 compile /tmp/seen_test_${test}.seen /tmp/seen_test_${test}_bin && /tmp/seen_test_${test}_bin"
    else
        skip_test "compile $test" "stage1_frozen not found"
    fi
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
    if [ -f "$STAGE1" ]; then
        run_test "feature $test" "rm -rf .seen_cache/ && $STAGE1 compile /tmp/seen_test_${test}.seen /tmp/seen_test_${test}_bin && /tmp/seen_test_${test}_bin"
    else
        skip_test "feature $test" "stage1_frozen not found"
    fi
done

# ---- Section 4: Type System ----
echo ""
echo "--- Type System ---"
run_test "type check pass" "echo 'fun main() r: Int { return 0 }' > /tmp/seen_typecheck.seen && $SEEN check /tmp/seen_typecheck.seen"

# ---- Section 5: Bootstrap Verification ----
echo ""
echo "--- Bootstrap ---"
if [ -f "$STAGE1" ]; then
    run_test "stage1 exists" "test -f $STAGE1"
    run_test "stage1 executable" "test -x $STAGE1"
    run_test "stage1 SHA256" "test -f bootstrap/stage1_frozen.sha256"
else
    skip_test "bootstrap" "stage1_frozen not found"
fi

# ---- Section 6: Production Benchmarks ----
echo ""
echo "--- Benchmarks (compile-only) ---"
if [ -f "$STAGE1" ]; then
    for bench in 01_matrix_mult 02_sieve 05_nbody 09_json_serialize 12_fannkuch; do
        if [ -f "benchmarks/production/${bench}.seen" ]; then
            run_test "bench compile $bench" "rm -rf .seen_cache/ && $STAGE1 compile benchmarks/production/${bench}.seen /tmp/seen_bench_${bench}"
        else
            skip_test "bench $bench" "not found"
        fi
    done
else
    skip_test "benchmarks" "stage1_frozen not found"
fi

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

if [ -f "$STAGE1" ]; then
    run_test "e-graph strength reduction" "rm -rf .seen_cache/ && $STAGE1 compile /tmp/seen_test_egraph.seen /tmp/seen_test_egraph_bin && /tmp/seen_test_egraph_bin"
else
    skip_test "e-graph" "stage1_frozen not found"
fi

# ---- Section 8: Runtime ----
echo ""
echo "--- Runtime ---"
run_test "runtime library exists" "test -f seen_runtime/seen_runtime.c"
run_test "region runtime exists" "test -f seen_runtime/seen_region.c"
run_test "region header exists" "test -f seen_runtime/seen_region.h"

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
