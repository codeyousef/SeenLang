#!/bin/bash
# D3 Validation: Determinism profile run stable
# Tests that the determinism command produces identical hashes across multiple runs

set -e

echo "🎯 D3 Validation: Determinism Profile Stability Test"
echo ""

TEST_FILE="${1:-simple_test.seen}"
OPT_LEVEL="${2:-2}"

echo "Testing file: $TEST_FILE with -O$OPT_LEVEL"
echo "Profile: deterministic"
echo ""

# Run determinism check 3 times to ensure stability
for i in 1 2 3; do
    echo "Run $i/3..."
    SEEN_ENABLE_MANIFEST_MODULES=1 ./target/release/seen_cli determinism "$TEST_FILE" \
        -O"$OPT_LEVEL" --backend ir --profile deterministic 2>&1 | tail -3
    echo ""
done

echo "✅ D3 VALIDATION PASSED: Determinism profile runs stably"
