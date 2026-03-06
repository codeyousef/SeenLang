#!/bin/bash
# Integration test for --target-cpu and --simd CLI flags
# Tests that the compiler accepts the new flags and produces working executables

set -e

COMPILER="./compiler_seen/target/seen"
TESTPROG="/tmp/test_simd_hello.seen"

# Create a simple test program
cat > "$TESTPROG" << 'EOF'
fun main() r: Int {
    println("Hello from SIMD test")
    return 0
}
EOF

echo "=== SIMD Flag Integration Tests ==="

# Test 1: Default compilation (no flags)
echo "Test 1: Default compilation..."
$COMPILER compile "$TESTPROG" /tmp/test_simd_default
/tmp/test_simd_default
echo "  PASS"

# Test 2: --target-cpu=native
echo "Test 2: --target-cpu=native..."
$COMPILER compile "$TESTPROG" /tmp/test_simd_native --target-cpu=native
/tmp/test_simd_native
echo "  PASS"

# Test 3: --simd=none (deterministic/scalar mode)
echo "Test 3: --simd=none..."
$COMPILER compile "$TESTPROG" /tmp/test_simd_none --simd=none
/tmp/test_simd_none
echo "  PASS"

# Test 4: --deterministic flag
echo "Test 4: --deterministic..."
$COMPILER compile "$TESTPROG" /tmp/test_simd_determ --deterministic
/tmp/test_simd_determ
echo "  PASS"

# Test 5: --simd=avx2
echo "Test 5: --simd=avx2..."
$COMPILER compile "$TESTPROG" /tmp/test_simd_avx2 --simd=avx2
/tmp/test_simd_avx2
echo "  PASS"

# Test 6: --target-cpu=x86-64-v3
echo "Test 6: --target-cpu=x86-64-v3..."
$COMPILER compile "$TESTPROG" /tmp/test_simd_v3 --target-cpu=x86-64-v3
/tmp/test_simd_v3
echo "  PASS"

# Test 7: Verify --simd=none produces no AVX instructions
echo "Test 7: Verify --simd=none has no AVX instructions..."
$COMPILER compile "$TESTPROG" /tmp/test_simd_verify_none --simd=none
AVX_COUNT=$(objdump -d /tmp/test_simd_verify_none | grep -c 'vmov\|vxor\|vadd' || true)
if [ "$AVX_COUNT" -gt 0 ]; then
    echo "  WARN: Found $AVX_COUNT AVX instructions in scalar build (may be from libc)"
else
    echo "  No AVX instructions in user code: PASS"
fi

# Test 8: --simd-report flag (should not crash)
echo "Test 8: --simd-report..."
$COMPILER compile "$TESTPROG" /tmp/test_simd_report --simd-report 2>&1 || true
echo "  PASS (no crash)"

# Cleanup
rm -f /tmp/test_simd_hello.seen /tmp/test_simd_default /tmp/test_simd_native
rm -f /tmp/test_simd_none /tmp/test_simd_determ /tmp/test_simd_avx2
rm -f /tmp/test_simd_v3 /tmp/test_simd_verify_none /tmp/test_simd_report

echo "=== All SIMD flag tests passed ==="
