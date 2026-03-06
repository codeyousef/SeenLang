#!/bin/bash
# WASM Backend Test Script
# Tests the WASM and UWW code generation targets

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
COMPILER="$PROJECT_ROOT/target-wsl/release/seen_cli"
TEST_DIR="$SCRIPT_DIR"
OUTPUT_DIR="$TEST_DIR/output"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo "========================================="
echo "WASM Backend Tests"
echo "========================================="
echo ""

# Check prerequisites
check_prerequisites() {
    echo "Checking prerequisites..."

    if ! command -v wat2wasm &> /dev/null; then
        echo -e "${YELLOW}Warning: wat2wasm not found. Install wabt: sudo apt install wabt${NC}"
        echo "Some tests will be skipped."
        return 1
    fi

    if ! command -v wasm-validate &> /dev/null; then
        echo -e "${YELLOW}Warning: wasm-validate not found.${NC}"
    fi

    if [ ! -f "$COMPILER" ]; then
        echo -e "${RED}Error: Compiler not found at $COMPILER${NC}"
        echo "Build the compiler first: cd rust_backup && cargo build --release"
        exit 1
    fi

    echo -e "${GREEN}Prerequisites OK${NC}"
    return 0
}

# Test 1: Basic WASM compilation
test_basic_wasm() {
    echo ""
    echo "Test 1: Basic WASM Compilation"
    echo "------------------------------"

    local source="$TEST_DIR/test_wasm_basic.seen"
    local output="$OUTPUT_DIR/test_basic"

    echo "Compiling: $source -> $output.wasm"

    if $COMPILER build "$source" -t wasm -o "$output" 2>&1; then
        echo -e "${GREEN}PASS: Compilation successful${NC}"

        # Check output files exist
        if [ -f "$output.wasm" ]; then
            echo -e "${GREEN}PASS: WASM binary created${NC}"
            ls -la "$output.wasm"
        else
            echo -e "${RED}FAIL: WASM binary not found${NC}"
            return 1
        fi

        # Validate WASM
        if command -v wasm-validate &> /dev/null; then
            if wasm-validate "$output.wasm" 2>&1; then
                echo -e "${GREEN}PASS: WASM validation passed${NC}"
            else
                echo -e "${RED}FAIL: WASM validation failed${NC}"
                return 1
            fi
        fi
    else
        echo -e "${RED}FAIL: Compilation failed${NC}"
        return 1
    fi
}

# Test 2: UWW deterministic compilation
test_uww_determinism() {
    echo ""
    echo "Test 2: UWW Deterministic Compilation"
    echo "--------------------------------------"

    local source="$TEST_DIR/test_uww_determinism.seen"
    local output1="$OUTPUT_DIR/test_uww_1"
    local output2="$OUTPUT_DIR/test_uww_2"

    echo "Compiling twice to verify determinism..."

    # First compilation
    if ! $COMPILER build "$source" -t uww -o "$output1" 2>&1; then
        echo -e "${RED}FAIL: First compilation failed${NC}"
        return 1
    fi

    # Second compilation
    if ! $COMPILER build "$source" -t uww -o "$output2" 2>&1; then
        echo -e "${RED}FAIL: Second compilation failed${NC}"
        return 1
    fi

    # Compare SHA-256 hashes
    if [ -f "$output1.wasm" ] && [ -f "$output2.wasm" ]; then
        local hash1=$(sha256sum "$output1.wasm" | cut -d' ' -f1)
        local hash2=$(sha256sum "$output2.wasm" | cut -d' ' -f1)

        echo "Hash 1: $hash1"
        echo "Hash 2: $hash2"

        if [ "$hash1" = "$hash2" ]; then
            echo -e "${GREEN}PASS: Deterministic output (hashes match)${NC}"
        else
            echo -e "${RED}FAIL: Non-deterministic output (hashes differ)${NC}"
            return 1
        fi
    else
        echo -e "${RED}FAIL: WASM files not found${NC}"
        return 1
    fi
}

# Test 3: UWW float rejection
test_uww_float_rejection() {
    echo ""
    echo "Test 3: UWW Float Type Rejection"
    echo "---------------------------------"

    local source="$TEST_DIR/test_uww_float_rejection.seen"
    local output="$OUTPUT_DIR/test_float"

    echo "Compiling (should fail due to float types)..."

    if $COMPILER build "$source" -t uww -o "$output" 2>&1; then
        echo -e "${RED}FAIL: Compilation should have failed (floats used)${NC}"
        return 1
    else
        echo -e "${GREEN}PASS: Compilation correctly rejected float types${NC}"
    fi
}

# Test 4: Check no WASI imports in UWW
test_uww_no_wasi() {
    echo ""
    echo "Test 4: UWW No WASI Imports"
    echo "---------------------------"

    local wasm="$OUTPUT_DIR/test_uww_1.wasm"

    if [ ! -f "$wasm" ]; then
        echo -e "${YELLOW}SKIP: UWW WASM not available${NC}"
        return 0
    fi

    echo "Checking for WASI imports..."

    if command -v wasm-objdump &> /dev/null; then
        if wasm-objdump -x "$wasm" | grep -qi "wasi"; then
            echo -e "${RED}FAIL: Found WASI imports in UWW binary${NC}"
            wasm-objdump -x "$wasm" | grep -i "wasi"
            return 1
        else
            echo -e "${GREEN}PASS: No WASI imports found${NC}"
        fi
    else
        echo -e "${YELLOW}SKIP: wasm-objdump not available${NC}"
    fi
}

# Test 5: Check file size
test_file_size() {
    echo ""
    echo "Test 5: Output Size Check"
    echo "-------------------------"

    local wasm="$OUTPUT_DIR/test_basic.wasm"

    if [ ! -f "$wasm" ]; then
        echo -e "${YELLOW}SKIP: WASM file not available${NC}"
        return 0
    fi

    local size=$(stat -c%s "$wasm" 2>/dev/null || stat -f%z "$wasm" 2>/dev/null)
    local max_size=$((1024 * 1024))  # 1MB

    echo "WASM size: $size bytes"

    if [ "$size" -lt "$max_size" ]; then
        echo -e "${GREEN}PASS: Size under 1MB limit${NC}"
    else
        echo -e "${RED}FAIL: Size exceeds 1MB limit${NC}"
        return 1
    fi
}

# Run all tests
run_all_tests() {
    local passed=0
    local failed=0
    local skipped=0

    check_prerequisites || true

    if test_basic_wasm; then
        ((passed++))
    else
        ((failed++))
    fi

    if test_uww_determinism; then
        ((passed++))
    else
        ((failed++))
    fi

    # Float rejection test - may be skipped if type checker isn't enforcing yet
    if test_uww_float_rejection; then
        ((passed++))
    else
        ((failed++))
    fi

    if test_uww_no_wasi; then
        ((passed++))
    else
        ((failed++))
    fi

    if test_file_size; then
        ((passed++))
    else
        ((failed++))
    fi

    echo ""
    echo "========================================="
    echo "Test Results"
    echo "========================================="
    echo -e "Passed: ${GREEN}$passed${NC}"
    echo -e "Failed: ${RED}$failed${NC}"
    echo ""

    if [ "$failed" -gt 0 ]; then
        exit 1
    fi
}

# Main
run_all_tests
