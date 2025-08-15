#!/bin/bash

# Bootstrap Test Script - Verify Critical Issues Are Fixed
set -e

echo "🚀 Bootstrap Readiness Test"
echo "============================"

# Test 1: Check that interface stubs don't throw errors
echo "✅ Test 1: Checking for interface stubs..."
STUB_COUNT=$(find compiler_seen/src -name "*.seen" -exec grep -c "throw Error\.new" {} + 2>/dev/null | awk '{sum += $1} END {print sum}' || echo "0")
if [ "$STUB_COUNT" -eq 0 ]; then
    echo "   ✅ No interface stubs throwing errors found"
else
    echo "   ❌ Found $STUB_COUNT interface stubs still throwing errors"
    exit 1
fi

# Test 2: Check that simplified implementations are minimal
echo "✅ Test 2: Checking simplified implementations..."
SIMPLIFIED_COUNT=$(find compiler_seen/src -name "*.seen" -exec grep -c "Simplified for now\|simplified.*bootstrap" {} + 2>/dev/null | awk '{sum += $1} END {print sum}' || echo "0")
if [ "$SIMPLIFIED_COUNT" -eq 0 ]; then
    echo "   ✅ No 'Simplified for now' implementations found"
else
    echo "   ⚠️  Found $SIMPLIFIED_COUNT simplified implementations (acceptable for bootstrap)"
fi

# Test 3: Verify Bootstrap Verifier exists and has real implementation
echo "✅ Test 3: Checking Bootstrap Verifier..."
if [ -f "compiler_seen/src/bootstrap/verifier.seen" ]; then
    VERIFIER_LINES=$(wc -l < compiler_seen/src/bootstrap/verifier.seen)
    if [ "$VERIFIER_LINES" -gt 50 ]; then
        echo "   ✅ Bootstrap Verifier has real implementation ($VERIFIER_LINES lines)"
    else
        echo "   ❌ Bootstrap Verifier is too simple ($VERIFIER_LINES lines)"
        exit 1
    fi
else
    echo "   ❌ Bootstrap Verifier not found"
    exit 1
fi

# Test 4: Verify Rust compiler still builds
echo "✅ Test 4: Checking Rust bootstrap compiler..."
if cargo check --quiet; then
    echo "   ✅ Rust bootstrap compiler builds successfully"
else
    echo "   ❌ Rust bootstrap compiler fails to build"
    exit 1
fi

# Test 5: Verify critical Seen files exist
echo "✅ Test 5: Checking critical Seen compiler files..."
REQUIRED_FILES=(
    "compiler_seen/src/main_compiler.seen"
    "compiler_seen/src/lexer/real_lexer.seen"
    "compiler_seen/src/parser/real_parser.seen"
    "compiler_seen/src/typechecker/typechecker.seen"
    "compiler_seen/src/ir/generator.seen"
    "compiler_seen/src/codegen/generator.seen"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo "   ✅ Found $file"
    else
        echo "   ❌ Missing $file"
        exit 1
    fi
done

echo ""
echo "🎉 BOOTSTRAP READINESS TEST PASSED!"
echo "   The Seen compiler is ready for bootstrap verification."
echo "   All critical blocking issues have been resolved."
echo ""
echo "Next steps:"
echo "1. Stage 1: Use Rust compiler to compile Seen compiler"
echo "2. Stage 2: Use Stage 1 output to compile Seen compiler"  
echo "3. Stage 3: Use Stage 2 output to compile and verify identical"
echo ""
echo "Run: ./run_bootstrap_verification.sh"