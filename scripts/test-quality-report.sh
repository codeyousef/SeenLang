#!/usr/bin/env bash

# Script to generate a comprehensive test quality report

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   Seen Language Test Quality Report    ${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Function to check if command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# 1. Test Count and Organization
echo -e "${CYAN}1. Test Statistics${NC}"
echo "-------------------"

# Count total tests
TOTAL_TESTS=$(cargo test --all -- --list 2>/dev/null | grep -E "^test " | wc -l || echo "0")
echo "Total tests: $TOTAL_TESTS"

# Count tests per crate
echo -e "\nTests per crate:"
for crate in seen_lexer seen_parser seen_typechecker seen_interpreter seen_ir seen_cli seen_compiler; do
    if [ -d "$crate" ]; then
        COUNT=$(cargo test -p $crate -- --list 2>/dev/null | grep -E "^test " | wc -l || echo "0")
        printf "  %-20s %d\n" "$crate:" "$COUNT"
    fi
done

# Count test types
echo -e "\nTest types:"
UNIT_TESTS=$(find . -path "*/src/*" -name "*test*.rs" -o -path "*/src/*/tests.rs" | wc -l)
INTEGRATION_TESTS=$(find . -path "*/tests/*" -name "*.rs" | wc -l)
echo "  Unit test files:        $UNIT_TESTS"
echo "  Integration test files: $INTEGRATION_TESTS"

# 2. Code Coverage
echo -e "\n${CYAN}2. Code Coverage${NC}"
echo "----------------"

if command_exists cargo-tarpaulin; then
    if [ -f "target/coverage/tarpaulin-report.json" ]; then
        echo "Latest coverage report found"
        # Would extract coverage % from JSON if jq is available
    else
        echo -e "${YELLOW}No recent coverage report. Run: ./scripts/coverage.sh${NC}"
    fi
else
    echo -e "${YELLOW}cargo-tarpaulin not installed${NC}"
fi

# 3. Mutation Testing Score
echo -e "\n${CYAN}3. Mutation Testing${NC}"
echo "-------------------"

if command_exists cargo-mutants; then
    if [ -f "target/mutants/mutants.log" ]; then
        echo "Latest mutation report found"
        # Extract score from log
        SCORE=$(grep -E "Mutation score:" target/mutants/mutants.log | grep -oE "[0-9.]+" || echo "N/A")
        echo "Mutation score: ${SCORE}%"
    else
        echo -e "${YELLOW}No recent mutation report. Run: ./scripts/mutation-test.sh${NC}"
    fi
else
    echo -e "${YELLOW}cargo-mutants not installed${NC}"
fi

# 4. Test Performance
echo -e "\n${CYAN}4. Test Performance${NC}"
echo "-------------------"

# Run tests with timing
echo "Running quick performance check..."
START_TIME=$(date +%s)
cargo test --all --release --quiet > /dev/null 2>&1
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
echo "Total test suite runtime: ${DURATION}s"

if [ $DURATION -lt 30 ]; then
    echo -e "${GREEN}✓ Tests run quickly${NC}"
elif [ $DURATION -lt 60 ]; then
    echo -e "${YELLOW}⚠ Tests are somewhat slow${NC}"
else
    echo -e "${RED}✗ Tests are too slow${NC}"
fi

# 5. Test Organization Quality
echo -e "\n${CYAN}5. Test Organization${NC}"
echo "--------------------"

# Check for test specifications
SPEC_COUNT=$(find . -name "*test*spec*.md" -o -name "*spec*.md" | grep -i test | wc -l)
echo "Test specification files: $SPEC_COUNT"

# Check for test utilities
if [ -f "tests/common/mod.rs" ] || [ -f "tests/common/test_utils.rs" ]; then
    echo -e "${GREEN}✓ Test utilities module exists${NC}"
else
    echo -e "${YELLOW}⚠ No common test utilities found${NC}"
fi

# Check for property tests
PROPTEST_COUNT=$(grep -r "proptest!" . --include="*.rs" 2>/dev/null | wc -l || echo "0")
if [ $PROPTEST_COUNT -gt 0 ]; then
    echo -e "${GREEN}✓ Property-based tests found: $PROPTEST_COUNT${NC}"
else
    echo -e "${YELLOW}⚠ No property-based tests found${NC}"
fi

# Check for benchmarks
BENCH_COUNT=$(find . -name "*.rs" -path "*/benches/*" | wc -l)
if [ $BENCH_COUNT -gt 0 ]; then
    echo -e "${GREEN}✓ Benchmark tests found: $BENCH_COUNT files${NC}"
else
    echo -e "${YELLOW}⚠ No benchmark tests found${NC}"
fi

# 6. Test Completeness
echo -e "\n${CYAN}6. Test Completeness${NC}"
echo "--------------------"

# Check for ignored tests
IGNORED_TESTS=$(cargo test --all -- --list 2>/dev/null | grep -E "^test.*ignored" | wc -l || echo "0")
if [ $IGNORED_TESTS -gt 0 ]; then
    echo -e "${YELLOW}⚠ Ignored tests: $IGNORED_TESTS${NC}"
else
    echo -e "${GREEN}✓ No ignored tests${NC}"
fi

# Check for TODO/FIXME in tests
TODO_COUNT=$(grep -r "TODO\|FIXME" . --include="*test*.rs" 2>/dev/null | wc -l || echo "0")
if [ $TODO_COUNT -gt 0 ]; then
    echo -e "${YELLOW}⚠ TODOs in tests: $TODO_COUNT${NC}"
else
    echo -e "${GREEN}✓ No TODOs in tests${NC}"
fi

# 7. Documentation Tests
echo -e "\n${CYAN}7. Documentation Tests${NC}"
echo "----------------------"

DOC_TEST_COUNT=$(cargo test --doc --all 2>&1 | grep -E "running [0-9]+ tests?" | grep -oE "[0-9]+" | head -1 || echo "0")
echo "Documentation tests: $DOC_TEST_COUNT"

# 8. Overall Quality Score
echo -e "\n${CYAN}8. Overall Test Quality Score${NC}"
echo "-----------------------------"

# Calculate score based on various metrics
SCORE=0
MAX_SCORE=0

# Test count (max 20 points)
MAX_SCORE=$((MAX_SCORE + 20))
if [ $TOTAL_TESTS -gt 100 ]; then
    SCORE=$((SCORE + 20))
elif [ $TOTAL_TESTS -gt 50 ]; then
    SCORE=$((SCORE + 15))
elif [ $TOTAL_TESTS -gt 20 ]; then
    SCORE=$((SCORE + 10))
elif [ $TOTAL_TESTS -gt 0 ]; then
    SCORE=$((SCORE + 5))
fi

# Test organization (max 20 points)
MAX_SCORE=$((MAX_SCORE + 20))
ORG_SCORE=0
[ -f "tests/common/mod.rs" ] && ORG_SCORE=$((ORG_SCORE + 5))
[ $SPEC_COUNT -gt 0 ] && ORG_SCORE=$((ORG_SCORE + 5))
[ $PROPTEST_COUNT -gt 0 ] && ORG_SCORE=$((ORG_SCORE + 5))
[ $BENCH_COUNT -gt 0 ] && ORG_SCORE=$((ORG_SCORE + 5))
SCORE=$((SCORE + ORG_SCORE))

# Test performance (max 10 points)
MAX_SCORE=$((MAX_SCORE + 10))
if [ $DURATION -lt 30 ]; then
    SCORE=$((SCORE + 10))
elif [ $DURATION -lt 60 ]; then
    SCORE=$((SCORE + 5))
fi

# No ignored tests or TODOs (max 10 points)
MAX_SCORE=$((MAX_SCORE + 10))
[ $IGNORED_TESTS -eq 0 ] && SCORE=$((SCORE + 5))
[ $TODO_COUNT -eq 0 ] && SCORE=$((SCORE + 5))

# Calculate percentage
PERCENTAGE=$(echo "scale=1; $SCORE * 100 / $MAX_SCORE" | bc)

echo -e "\nQuality Score: ${SCORE}/${MAX_SCORE} (${PERCENTAGE}%)"

# Grade
if (( $(echo "$PERCENTAGE >= 80" | bc -l) )); then
    echo -e "${GREEN}Grade: A - Excellent test quality!${NC}"
elif (( $(echo "$PERCENTAGE >= 70" | bc -l) )); then
    echo -e "${GREEN}Grade: B - Good test quality${NC}"
elif (( $(echo "$PERCENTAGE >= 60" | bc -l) )); then
    echo -e "${YELLOW}Grade: C - Acceptable test quality${NC}"
elif (( $(echo "$PERCENTAGE >= 50" | bc -l) )); then
    echo -e "${YELLOW}Grade: D - Needs improvement${NC}"
else
    echo -e "${RED}Grade: F - Poor test quality${NC}"
fi

# 9. Recommendations
echo -e "\n${CYAN}9. Recommendations${NC}"
echo "------------------"

if [ $TOTAL_TESTS -lt 50 ]; then
    echo "• Add more tests to improve coverage"
fi

if [ $SPEC_COUNT -eq 0 ]; then
    echo "• Create test specification documents"
fi

if [ $PROPTEST_COUNT -eq 0 ]; then
    echo "• Add property-based tests for better edge case coverage"
fi

if [ $BENCH_COUNT -eq 0 ]; then
    echo "• Add benchmark tests to track performance"
fi

if [ $IGNORED_TESTS -gt 0 ]; then
    echo "• Fix or remove ignored tests"
fi

if [ $TODO_COUNT -gt 0 ]; then
    echo "• Address TODOs in test code"
fi

echo -e "\n${BLUE}========================================${NC}"
echo -e "${BLUE}         End of Report                  ${NC}"
echo -e "${BLUE}========================================${NC}"