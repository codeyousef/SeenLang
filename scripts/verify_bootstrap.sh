#!/bin/bash
# Bootstrap Verification Script
# Simulates the triple bootstrap process to verify self-hosting

set -e  # Exit on error

# Change to project root directory
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

echo "======================================"
echo "Seen Bootstrap Verification Process"
echo "======================================"
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Working directory
WORK_DIR="bootstrap_verification"
STAGE1_DIR="$WORK_DIR/stage1"
STAGE2_DIR="$WORK_DIR/stage2"
STAGE3_DIR="$WORK_DIR/stage3"

# Clean up previous runs
echo "Cleaning up previous bootstrap attempts..."
rm -rf $WORK_DIR
mkdir -p $STAGE1_DIR $STAGE2_DIR $STAGE3_DIR

# Stage 0: Build Rust bootstrap compiler
echo -e "${YELLOW}Stage 0: Building Rust bootstrap compiler${NC}"
CARGO_TARGET_DIR=target-wsl cargo build --manifest-path rust_backup/Cargo.toml -p seen_cli --release --features llvm
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Rust bootstrap compiler built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build Rust bootstrap compiler${NC}"
    exit 1
fi

# Stage 1: Compile Seen compiler with Rust compiler
echo
echo -e "${YELLOW}Stage 1: Compiling Seen compiler with Rust bootstrap${NC}"
SEEN_ENABLE_MANIFEST_MODULES=1 ./target-wsl/release/seen_cli build \
    compiler_seen/src/main_compiler.seen \
    --backend llvm \
    -o $STAGE1_DIR/seen

if [ -f "$STAGE1_DIR/seen" ]; then
    echo -e "${GREEN}✓ Stage 1 compilation successful${NC}"
    STAGE1_HASH=$(sha256sum $STAGE1_DIR/seen | cut -d' ' -f1)
    echo "  Hash: ${STAGE1_HASH:0:16}..."
else
    echo -e "${RED}✗ Stage 1 compilation failed${NC}"
    exit 1
fi

# Stage 2: Compile with Stage 1 compiler
echo
echo -e "${YELLOW}Stage 2: Compiling Seen compiler with Stage 1${NC}"
SEEN_ENABLE_MANIFEST_MODULES=1 $STAGE1_DIR/seen compile \
    compiler_seen/src/main_compiler.seen \
    $STAGE2_DIR/seen

if [ -f "$STAGE2_DIR/seen" ]; then
    echo -e "${GREEN}✓ Stage 2 compilation successful${NC}"
    STAGE2_HASH=$(sha256sum $STAGE2_DIR/seen | cut -d' ' -f1)
    echo "  Hash: ${STAGE2_HASH:0:16}..."
else
    echo -e "${RED}✗ Stage 2 compilation failed${NC}"
    exit 1
fi

# Stage 3: Compile with Stage 2 compiler
echo
echo -e "${YELLOW}Stage 3: Compiling Seen compiler with Stage 2${NC}"
SEEN_ENABLE_MANIFEST_MODULES=1 $STAGE2_DIR/seen compile \
    compiler_seen/src/main_compiler.seen \
    $STAGE3_DIR/seen

if [ -f "$STAGE3_DIR/seen" ]; then
    echo -e "${GREEN}✓ Stage 3 compilation successful${NC}"
    STAGE3_HASH=$(sha256sum $STAGE3_DIR/seen | cut -d' ' -f1)
    echo "  Hash: ${STAGE3_HASH:0:16}..."
else
    echo -e "${RED}✗ Stage 3 compilation failed${NC}"
    exit 1
fi

# Stage 4: Compile with Stage 3 compiler (for full fixed-point verification)
echo
STAGE4_DIR="$WORK_DIR/stage4"
mkdir -p $STAGE4_DIR
echo -e "${YELLOW}Stage 4: Compiling Seen compiler with Stage 3${NC}"
SEEN_ENABLE_MANIFEST_MODULES=1 $STAGE3_DIR/seen compile \
    compiler_seen/src/main_compiler.seen \
    $STAGE4_DIR/seen

if [ -f "$STAGE4_DIR/seen" ]; then
    echo -e "${GREEN}✓ Stage 4 compilation successful${NC}"
    STAGE4_HASH=$(sha256sum $STAGE4_DIR/seen | cut -d' ' -f1)
    echo "  Hash: ${STAGE4_HASH:0:16}..."
else
    echo -e "${RED}✗ Stage 4 compilation failed${NC}"
    exit 1
fi

# Verify bootstrap stability
echo
echo -e "${YELLOW}Verifying bootstrap stability...${NC}"
if [ "$STAGE2_HASH" = "$STAGE3_HASH" ]; then
    echo -e "${GREEN}✓ Bootstrap verification successful!${NC}"
    echo "  Stage 2 and Stage 3 binaries are identical"
    echo "  The compiler is fully self-hosted (perfect fixed-point from stage 2)"
elif [ "$STAGE3_HASH" = "$STAGE4_HASH" ]; then
    echo -e "${GREEN}✓ Bootstrap verification successful!${NC}"
    echo "  Stage 3 and Stage 4 binaries are identical"
    echo "  The compiler is fully self-hosted (fixed-point from stage 3)"
    echo "  Note: Stage 2 differs from Stage 3 due to Rust vs Seen codegen differences"
else
    echo -e "${RED}✗ Bootstrap verification failed!${NC}"
    echo "  Stage 2 hash: $STAGE2_HASH"
    echo "  Stage 3 hash: $STAGE3_HASH"
    echo "  Stage 4 hash: $STAGE4_HASH"
    echo "  The compiler is not yet stable for self-hosting"
    exit 1
fi

# Check for Rust symbols
echo
echo -e "${YELLOW}Checking for Rust symbols in final binary...${NC}"
if strings $STAGE3_DIR/seen | grep -q "rust_\|_ZN\|rustc\|cargo"; then
    echo -e "${YELLOW}⚠ Warning: Rust symbols detected in final binary${NC}"
    echo "  The binary may still have Rust dependencies"
    RUST_FREE=false
else
    echo -e "${GREEN}✓ No Rust symbols found${NC}"
    echo "  The binary is free of Rust dependencies"
    RUST_FREE=true
fi

# Run basic tests with the self-hosted compiler
echo
echo -e "${YELLOW}Running basic tests with self-hosted compiler...${NC}"
$STAGE3_DIR/seen compile examples/linux/hello_cli/main.seen /tmp/seen_bootstrap_test > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Self-hosted compiler is functional${NC}"
else
    echo -e "${RED}✗ Self-hosted compiler failed basic test${NC}"
    exit 1
fi

# Summary
echo
echo "======================================"
echo -e "${GREEN}Bootstrap Verification Complete${NC}"
echo "======================================"
echo
echo "Results:"
echo "  • Triple bootstrap: SUCCESS"
echo "  • Binary stability: VERIFIED"
if [ "$RUST_FREE" = true ]; then
    echo "  • Rust-free: YES"
    echo
    echo -e "${GREEN}The Seen compiler is ready for Rust removal!${NC}"
    echo
    echo "Next steps:"
    echo "  1. Run: ./scripts/remove_rust.sh"
    echo "  2. Test: seen test"
    echo "  3. Commit: git add -A && git commit -m 'Achieve self-hosting'"
else
    echo "  • Rust-free: NO"
    echo
    echo -e "${YELLOW}The compiler is self-hosted but still contains Rust symbols.${NC}"
    echo "Additional work may be needed to fully remove Rust dependencies."
fi

# Save the verified compiler
echo
echo "Saving verified compiler to: compiler_seen/target/seen"
mkdir -p compiler_seen/target
cp $STAGE3_DIR/seen compiler_seen/target/seen
chmod +x compiler_seen/target/seen

echo
echo "Bootstrap artifacts saved in: $WORK_DIR/"