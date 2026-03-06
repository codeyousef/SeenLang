#!/usr/bin/env bash
# Baseline Test Runner for SeenLang
# Runs all existing tests to establish a baseline before adding new tests

set -e

COMPILER="./bootstrap/stage1_frozen_v3"
MEMORY_LIMIT="8G"
TEMP_DIR="/tmp/seen_test_outputs"

mkdir -p "$TEMP_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
TOTAL=0
PASSED=0
FAILED=0
SKIPPED=0

# Find all test files
TEST_FILES=$(find tests/misc_root_tests -name "test_*.seen" -type f | sort)

echo "================================================================"
echo "SEENLANG BASELINE TEST SUITE"
echo "================================================================"
echo "Running existing tests to establish baseline..."
echo ""

for TEST_FILE in $TEST_FILES; do
    TOTAL=$((TOTAL + 1))
    TEST_NAME=$(basename "$TEST_FILE" .seen)
    OUTPUT_BIN="$TEMP_DIR/$TEST_NAME"

    printf "[%3d] %-50s " "$TOTAL" "$TEST_NAME"

    # Compile the test
    if systemd-run --user --quiet --scope -p MemoryMax=$MEMORY_LIMIT \
        $COMPILER compile "$TEST_FILE" "$OUTPUT_BIN" 2>&1 > /dev/null; then

        # Run the test
        if $OUTPUT_BIN > /dev/null 2>&1; then
            EXIT_CODE=$?
            if [ $EXIT_CODE -eq 0 ]; then
                echo -e "${GREEN}PASS${NC}"
                PASSED=$((PASSED + 1))
            else
                echo -e "${RED}FAIL${NC} (exit code: $EXIT_CODE)"
                FAILED=$((FAILED + 1))
            fi
        else
            echo -e "${RED}FAIL${NC} (runtime error)"
            FAILED=$((FAILED + 1))
        fi
    else
        echo -e "${YELLOW}SKIP${NC} (compilation failed)"
        SKIPPED=$((SKIPPED + 1))
    fi
done

echo ""
echo "================================================================"
echo "BASELINE TEST RESULTS"
echo "================================================================"
echo "Total:   $TOTAL"
echo -e "Passed:  ${GREEN}$PASSED${NC}"
echo -e "Failed:  ${RED}$FAILED${NC}"
echo -e "Skipped: ${YELLOW}$SKIPPED${NC}"

if [ $FAILED -eq 0 ]; then
    echo ""
    echo -e "${GREEN}SUCCESS! All tests passed!${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}FAILURE! Some tests failed.${NC}"
    exit 1
fi
