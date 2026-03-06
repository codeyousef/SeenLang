#!/bin/bash
# Run all Linux platform E2E tests
# Usage: ./run_platform_linux_tests.sh
#
# Note: Cross-module imports are not fully working in the self-hosted compiler,
# so we use self-contained tests that verify constant values directly.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SEEN_COMPILER="$PROJECT_ROOT/compiler_seen/target/seen"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}=== Linux Platform E2E Tests ===${NC}"
echo ""

# Check for compiler
if [ ! -f "$SEEN_COMPILER" ]; then
    echo -e "${YELLOW}Self-hosted compiler not found at $SEEN_COMPILER${NC}"
    echo "Falling back to bootstrap compiler..."
    SEEN_COMPILER="$PROJECT_ROOT/target-wsl/release/seen_cli"
    if [ ! -f "$SEEN_COMPILER" ]; then
        SEEN_COMPILER="$PROJECT_ROOT/target/release/seen_cli"
    fi
fi

if [ ! -f "$SEEN_COMPILER" ]; then
    echo -e "${RED}No Seen compiler found!${NC}"
    exit 1
fi

echo "Using compiler: $SEEN_COMPILER"
echo ""

# Self-contained tests that work with the self-hosted compiler
# These don't rely on cross-module imports
TESTS=(
    "platform_linux_constants.seen"
)

PASSED=0
FAILED=0
SKIPPED=0

for test in "${TESTS[@]}"; do
    test_path="$SCRIPT_DIR/$test"
    test_name="${test%.seen}"
    output_binary="/tmp/$test_name"

    if [ ! -f "$test_path" ]; then
        echo -e "${YELLOW}SKIP${NC}: $test (file not found)"
        ((SKIPPED++))
        continue
    fi

    echo -n "Testing $test_name... "

    # Compile the test (use 'compile' for self-hosted, 'build' for bootstrap)
    compile_output=""
    if [[ "$SEEN_COMPILER" == *"seen_cli"* ]]; then
        compile_output=$(SEEN_ENABLE_MANIFEST_MODULES=1 "$SEEN_COMPILER" build "$test_path" -o "$output_binary" --backend llvm 2>&1)
    else
        # Self-hosted compiler outputs binary based on input path, not -o flag
        compile_output=$("$SEEN_COMPILER" compile "$test_path" "$output_binary" 2>&1)
    fi

    if [ $? -eq 0 ]; then
        # Run the test
        if "$output_binary" > /tmp/${test_name}_output.txt 2>&1; then
            # Check for failures in output
            if grep -q "TESTS FAILED" /tmp/${test_name}_output.txt; then
                echo -e "${RED}FAIL${NC}"
                cat /tmp/${test_name}_output.txt
                ((FAILED++))
            else
                echo -e "${GREEN}PASS${NC}"
                # Show summary
                grep "Passed:" /tmp/${test_name}_output.txt || true
                ((PASSED++))
            fi
        else
            echo -e "${RED}FAIL${NC} (runtime error)"
            cat /tmp/${test_name}_output.txt
            ((FAILED++))
        fi
        rm -f "$output_binary"
    else
        echo -e "${RED}FAIL${NC} (compilation error)"
        ((FAILED++))
    fi
done

echo ""
echo -e "${CYAN}=== Summary ===${NC}"
echo -e "Passed:  ${GREEN}$PASSED${NC}"
echo -e "Failed:  ${RED}$FAILED${NC}"
echo -e "Skipped: ${YELLOW}$SKIPPED${NC}"
echo ""

if [ $FAILED -gt 0 ]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi

echo -e "${GREEN}All Linux platform tests passed!${NC}"
exit 0
