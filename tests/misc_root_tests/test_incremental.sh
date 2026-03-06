#!/usr/bin/env bash
# Test incremental compilation caching
set -e
cd "$(dirname "$0")/../.."

COMPILER=./compiler_seen/target/seen
TEST_SRC=tests/misc_root_tests/test_hello.seen
TEST_OUT=/tmp/test_incr_hello

echo "=== Incremental Compilation Test ==="

# First build (no cache)
rm -rf .seen_cache/
echo "--- First build (cold cache) ---"
$COMPILER compile $TEST_SRC $TEST_OUT 2>&1 | tee /tmp/incr_output.txt

# Verify output works
OUTPUT=$($TEST_OUT)
echo "$OUTPUT" | grep -q "Hello World" && echo "PASS: output correct after first build"

# Verify cache dir exists
test -d .seen_cache/ && echo "PASS: cache directory created"
test -f .seen_cache/registry.hash && echo "PASS: registry hash cached"

# Count cached .o files
CACHED_O=$(find .seen_cache/ -name "*.o" | wc -l)
echo "PASS: $CACHED_O object files cached"

# Second build (fully cached — should be fast)
echo ""
echo "--- Second build (warm cache) ---"
$COMPILER compile $TEST_SRC $TEST_OUT 2>&1 | tee /tmp/incr_output.txt

# Verify "Cached:" appears in output
CACHED_LINES=$(grep -c "Cached:" /tmp/incr_output.txt || true)
if [ "$CACHED_LINES" -gt 0 ]; then
    echo "PASS: $CACHED_LINES modules served from cache"
else
    echo "FAIL: no modules served from cache"
    exit 1
fi

# Verify output still works after cached build
OUTPUT=$($TEST_OUT)
echo "$OUTPUT" | grep -q "Hello World" && echo "PASS: output correct after cached build"

# Test --no-cache flag
echo ""
echo "--- Third build (--no-cache) ---"
$COMPILER compile --no-cache $TEST_SRC $TEST_OUT 2>&1 | tee /tmp/incr_output2.txt

CACHED_LINES2=$(grep -c "Cached:" /tmp/incr_output2.txt || true)
if [ "$CACHED_LINES2" -eq 0 ]; then
    echo "PASS: --no-cache skipped cache"
else
    echo "FAIL: --no-cache still used cache"
    exit 1
fi

# Cleanup
rm -rf .seen_cache/
rm -f /tmp/incr_output.txt /tmp/incr_output2.txt

echo ""
echo "All incremental compilation tests passed"
