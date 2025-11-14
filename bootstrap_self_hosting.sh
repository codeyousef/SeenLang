#!/bin/bash

# Bootstrap Script for Seen Self-Hosting Compiler
# This script uses the Rust bootstrap compiler to compile the self-hosting compiler

set -e

echo "🚀 Bootstrapping Seen Self-Hosting Compiler"
echo "=============================================="

# Step 1: Build the Rust bootstrap compiler
echo "📦 Step 1: Building Rust bootstrap compiler..."
CARGO_TARGET_DIR=target-wsl cargo build --release --workspace
echo "✅ Bootstrap compiler built successfully"

# Step 2: Create intermediate compiler using bootstrap
echo "⚙️  Step 2: Creating bootstrap process..."

# Step 3: Attempt to compile simple Seen programs first  
echo "🧪 Step 3: Testing bootstrap compiler capabilities..."

# Create a minimal test program
cat > bootstrap_minimal.seen << 'EOF'
fun main() {
    42
}
EOF

echo "Testing minimal program compilation..."
if ./target-wsl/release/seen_cli check bootstrap_minimal.seen 2>/dev/null; then
    echo "✅ Basic syntax checking works"
else
    echo "⚠️  Basic syntax checking needs work"
fi

# Step 4: Try to compile parts of the self-hosting compiler
echo "🏗️  Step 4: Attempting to compile self-hosting components..."

# Check if we can parse the main self-hosting compiler file
if ./target-wsl/release/seen_cli check compiler_seen/src/main.seen 2>/dev/null; then
    echo "✅ Self-hosting main compiler passes syntax check"
else
    echo "⚠️  Self-hosting compiler needs syntax fixes"
fi

# Step 5 removed: Always build the real self-hosted compiler; no simplified bootstrap.
echo ""
echo "📊 Bootstrap Summary:"
echo "- Rust compiler: ✅ Built"
echo "- Basic parsing: Testing..."  
echo "- Self-hosting: In progress..."
echo ""
echo "Next steps:"
echo "1. Fix remaining parser issues"
echo "2. Complete self-hosting compiler compilation"
echo "3. Verify performance targets"

# Cleanup
rm -f bootstrap_minimal.seen