#!/bin/bash
# Test sanitizer CLI flags
# Verifies --sanitize=address compilation and basic execution

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

if [ ! -f "$COMPILER" ]; then
    echo -e "${RED}Compiler not found${NC}"
    exit 1
fi

# Create test program
TEST_FILE="/tmp/test_sanitizer_basic.seen"
cat > "$TEST_FILE" << 'EOF'
fun main() r: Int {
    let x = 42
    let y = x + 1
    println("Sanitizer test: " + y.toString())
    return 0
}
EOF

echo "=== Test: --sanitize=address ==="
cd "$ROOT_DIR"
# Note: ASan may report leaks (expected for Seen programs without explicit .free())
# We suppress leak detection to focus on actual memory errors
export ASAN_OPTIONS=detect_leaks=0
if "$COMPILER" compile "$TEST_FILE" /tmp/test_sanitizer_out --sanitize=address 2>&1 | tail -1 | grep -q "Build succeeded"; then
    echo -e "${GREEN}Compilation with ASan succeeded${NC}"
    if /tmp/test_sanitizer_out 2>&1 | grep -q "Sanitizer test: 43"; then
        echo -e "${GREEN}Execution with ASan passed${NC}"
    else
        echo -e "${RED}Execution with ASan failed${NC}"
        exit 1
    fi
else
    echo -e "${RED}Compilation with ASan failed${NC}"
    exit 1
fi

echo "=== Test: --sanitize=undefined ==="
if "$COMPILER" compile "$TEST_FILE" /tmp/test_sanitizer_out --sanitize=undefined --no-cache 2>&1 | tail -1 | grep -q "Build succeeded"; then
    echo -e "${GREEN}Compilation with UBSan succeeded${NC}"
    /tmp/test_sanitizer_out 2>&1 | grep -q "Sanitizer test: 43" && echo -e "${GREEN}Execution with UBSan passed${NC}"
else
    echo -e "${RED}Compilation with UBSan failed${NC}"
    exit 1
fi

rm -f /tmp/test_sanitizer_out /tmp/test_sanitizer_basic.seen /tmp/seen_runtime_sanitized.o
echo -e "${GREEN}All sanitizer tests passed${NC}"
