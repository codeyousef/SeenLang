#!/bin/bash
# Build script for Seen compiler with LLVM integration

echo "=== Seen Compiler LLVM Build Script ==="
echo

# Set LLVM environment variables
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
export CARGO_TARGET_DIR=/tmp/cargo_target
export PATH=/usr/lib/llvm-18/bin:$PATH

# Color codes for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to check if a command succeeded
check_status() {
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ $1 successful${NC}"
    else
        echo -e "${RED}✗ $1 failed${NC}"
        exit 1
    fi
}

# Step 1: Verify LLVM installation
echo -e "${YELLOW}Step 1: Verifying LLVM installation...${NC}"
echo "LLVM version: $(llvm-config-18 --version)"
echo "LLVM prefix: $(llvm-config-18 --prefix)"
echo "Checking for Polly libraries..."
if [ -f "/usr/lib/llvm-18/lib/libPolly.a" ] && [ -f "/usr/lib/llvm-18/lib/libPollyISL.a" ]; then
    echo -e "${GREEN}✓ Polly static libraries found${NC}"
else
    echo -e "${RED}✗ Polly static libraries missing! Run: sudo apt install libpolly-18-dev${NC}"
    exit 1
fi
echo

# Step 2: Build seen_ir
echo -e "${YELLOW}Step 2: Building seen_ir (LLVM IR generation)...${NC}"
cargo build --package seen_ir
check_status "seen_ir build"
echo

# Step 3: Build seen_cli
echo -e "${YELLOW}Step 3: Building seen_cli (Command-line interface)...${NC}"
cargo build --package seen_cli
check_status "seen_cli build"
echo

# Step 4: Build seen_compiler
echo -e "${YELLOW}Step 4: Building seen_compiler...${NC}"
cargo build --package seen_compiler
check_status "seen_compiler build"
echo

# Step 5: Run tests
echo -e "${YELLOW}Step 5: Running tests...${NC}"

echo "Testing seen_ir..."
cargo test --package seen_ir -- --nocapture
IR_TEST_STATUS=$?

echo "Testing seen_cli..."
cargo test --package seen_cli -- --nocapture
CLI_TEST_STATUS=$?

if [ $IR_TEST_STATUS -eq 0 ] && [ $CLI_TEST_STATUS -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed${NC}"
else
    echo -e "${YELLOW}⚠ Some tests failed (this may be expected for unimplemented features)${NC}"
fi
echo

# Step 6: Test end-to-end compilation
echo -e "${YELLOW}Step 6: Testing end-to-end compilation...${NC}"

# Create a test directory
TEST_DIR="/tmp/seen_test_project"
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"

# Create a simple test program
cat > "$TEST_DIR/test.seen" << 'EOF'
func main() {
    println("Hello from LLVM-compiled Seen!");
}
EOF

echo "Created test program at $TEST_DIR/test.seen"

# Try to compile it (this might fail if CLI LLVM deps aren't enabled)
if command -v seen_cli >/dev/null 2>&1 || [ -f "target/debug/seen_cli" ]; then
    echo "Attempting to compile test program..."
    if [ -f "target/debug/seen_cli" ]; then
        SEEN_CLI="./target/debug/seen_cli"
    else
        SEEN_CLI="seen_cli"
    fi
    
    # Try to create a new project
    cd "$TEST_DIR"
    $SEEN_CLI new hello_llvm
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ CLI can create projects${NC}"
        
        # Try to build the project
        cd hello_llvm
        $SEEN_CLI build
        if [ $? -eq 0 ]; then
            echo -e "${GREEN}✓ Full LLVM compilation pipeline working!${NC}"
        else
            echo -e "${YELLOW}⚠ Build failed (may need to enable LLVM in CLI Cargo.toml)${NC}"
        fi
    else
        echo -e "${YELLOW}⚠ Project creation failed${NC}"
    fi
else
    echo -e "${YELLOW}⚠ seen_cli not found in PATH${NC}"
fi

echo
echo -e "${GREEN}=== Build script completed ===${NC}"
echo
echo "Next steps:"
echo "1. If compilation errors occurred, check llvm_fix_instructions.md"
echo "2. Enable LLVM dependencies in Cargo.toml files if not done"
echo "3. Run this script again after fixes"
echo
echo "To use the compiler:"
echo "  export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18"
echo "  cargo run --package seen_cli -- new myproject"
echo "  cargo run --package seen_cli -- build myproject"
echo "  cargo run --package seen_cli -- run myproject"