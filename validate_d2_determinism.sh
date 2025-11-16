#!/bin/bash
# D2 Validation: Stage2 == Stage3 Deterministic Hash Equality
# Validates that IR generation produces identical hashes for Stage2 and Stage3

set -e

echo "🎯 D2 Validation: Stage2 == Stage3 Hash Equality Test"
echo ""

TEST_FILES=(
    "simple_test.seen"
    "minimal_bootstrap_test.seen"
)

ALL_PASSED=true

for TEST_FILE in "${TEST_FILES[@]}"; do
    if [ ! -f "$TEST_FILE" ]; then
        echo "⚠️  Skipping $TEST_FILE (not found)"
        continue
    fi
    
    echo "Testing: $TEST_FILE"
    
    # Run determinism check
    OUTPUT=$(SEEN_ENABLE_MANIFEST_MODULES=1 ./target/release/seen_cli determinism "$TEST_FILE" \
        -O2 --backend ir --profile deterministic 2>&1)
    
    STAGE2=$(echo "$OUTPUT" | grep "Stage2 hash:" | cut -d' ' -f3)
    STAGE3=$(echo "$OUTPUT" | grep "Stage3 hash:" | cut -d' ' -f3)
    
    if [ "$STAGE2" = "$STAGE3" ] && [ -n "$STAGE2" ]; then
        echo "  ✅ PASSED: $STAGE2"
    else
        echo "  ❌ FAILED: Stage2=$STAGE2, Stage3=$STAGE3"
        ALL_PASSED=false
    fi
    echo ""
done

if [ "$ALL_PASSED" = true ]; then
    echo "✅ D2 VALIDATION PASSED: All tests show Stage2 == Stage3 hash equality"
    exit 0
else
    echo "❌ D2 VALIDATION FAILED: Some tests failed"
    exit 1
fi
