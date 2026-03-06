#!/bin/bash
# Bootstrap the Seen compiler from the Rust backup compiler
# This script attempts to build a new stage1 Seen compiler using the Rust bootstrap

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUST_BACKUP="$PROJECT_ROOT/rust_backup"

echo "=== Seen Compiler Bootstrap from Rust ==="
echo "Project root: $PROJECT_ROOT"
echo "Rust backup: $RUST_BACKUP"

# Check if rust_backup exists
if [ ! -d "$RUST_BACKUP" ]; then
    echo "ERROR: rust_backup directory not found"
    echo "Try: git checkout d6a1271 -- rust_backup/"
    exit 1
fi

# Build the Rust compiler with LLVM support
echo ""
echo "=== Building Rust bootstrap compiler with LLVM support ==="
cd "$RUST_BACKUP"
cargo build -p seen_cli --release --features llvm

RUST_CLI="$RUST_BACKUP/target/release/seen_cli"
if [ ! -x "$RUST_CLI" ]; then
    echo "ERROR: Failed to build Rust CLI"
    exit 1
fi

echo "Rust CLI built: $RUST_CLI"

# Test with a simple program
echo ""
echo "=== Testing Rust compiler with simple program ==="
cd "$PROJECT_ROOT"
TEST_FILE="$PROJECT_ROOT/tests/misc_root_tests/test_hello.seen"
TEST_OUT="/tmp/seen_test_hello"

"$RUST_CLI" build "$TEST_FILE" -o "$TEST_OUT" --backend llvm
"$TEST_OUT"
echo "Simple test passed!"

# Attempt to compile the Seen compiler
echo ""
echo "=== Attempting to compile Seen compiler ==="
COMPILER_SRC="$PROJECT_ROOT/compiler_seen/src/main_compiler.seen"
STAGE2_OUT="$PROJECT_ROOT/stage2_from_rust"

echo "NOTE: The Rust LLVM backend has issues with complex generic types."
echo "This may fail with type-related errors."
echo ""

if "$RUST_CLI" build "$COMPILER_SRC" -o "$STAGE2_OUT" --backend llvm 2>&1 | tee /tmp/bootstrap_log.txt; then
    echo ""
    echo "=== SUCCESS ==="
    echo "New Seen compiler built at: $STAGE2_OUT"
    echo "Test with: $STAGE2_OUT compile <file.seen>"
else
    echo ""
    echo "=== Bootstrap failed ==="
    echo "See /tmp/bootstrap_log.txt for details"
    echo ""
    echo "Known issue: Rust LLVM backend struggles with generic types like BTreeMap"
    echo "Alternative approaches:"
    echo "  1. Simplify the compiler to avoid complex generics"
    echo "  2. Fix the Rust LLVM backend in rust_backup/seen_ir/src/llvm_backend.rs"
    echo "  3. Find an earlier working stage1 compiler"
    exit 1
fi
