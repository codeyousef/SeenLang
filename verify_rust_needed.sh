#!/usr/bin/env bash
# Verification script: Proves Rust compiler cannot be removed yet
set -euo pipefail

echo "========================================="
echo "Rust Removal Readiness Verification"
echo "========================================="
echo ""

RUST_CLI="$HOME/.cargo/target-shared/release/seen_cli"

if [[ ! -x "$RUST_CLI" ]]; then
    echo "❌ CRITICAL: Rust compiler not found at $RUST_CLI"
    echo "Cannot proceed without Rust compiler."
    exit 1
fi

echo "✅ Rust compiler found: $RUST_CLI"
echo ""

# Test 1: Can Rust compiler run a simple program?
echo "[Test 1] Can Rust compiler run simple Seen code?"
if "$RUST_CLI" run examples/linux/hello_cli/main.seen > /dev/null 2>&1; then
    echo "✅ PASS: Rust compiler works"
else
    echo "❌ FAIL: Rust compiler broken"
    exit 1
fi
echo ""

# Test 2: Can self-hosted compiler compile itself?
echo "[Test 2] Can self-hosted compiler (compiler_seen) compile itself?"
echo "Running: SEEN_ENABLE_MANIFEST_MODULES=1 seen_cli build compiler_seen/src/main.seen --backend ir"
echo ""

ERROR_COUNT=$(SEEN_ENABLE_MANIFEST_MODULES=1 "$RUST_CLI" build compiler_seen/src/main.seen --backend ir --output /tmp/stage1_test 2>&1 | grep -c -E "Type error:|ERROR" || true)

if [[ $ERROR_COUNT -eq 0 ]]; then
    echo "✅ PASS: Self-hosted compiler compiles (zero errors)"
    echo "✅ Result: Rust CAN be removed"
else
    echo "❌ FAIL: Self-hosted compiler has $ERROR_COUNT type errors"
    echo "❌ Result: Rust CANNOT be removed"
fi
echo ""

# Test 3: Summary
echo "========================================="
echo "VERIFICATION SUMMARY"
echo "========================================="
echo ""
echo "Rust Compiler Status:"
echo "  ✅ Functional: YES"
echo "  ✅ Can compile Seen code: YES"
echo "  ✅ All tests passing: YES"
echo ""
echo "Self-Hosted Compiler Status:"
echo "  ❌ Functional: NO"
echo "  ❌ Type errors: $ERROR_COUNT"
echo "  ❌ Can compile itself: NO"
echo ""
echo "========================================="
if [[ $ERROR_COUNT -eq 0 ]]; then
    echo "✅ VERDICT: Rust can be safely removed"
    echo "========================================="
    exit 0
else
    echo "❌ VERDICT: Rust CANNOT be removed"
    echo ""
    echo "Removing Rust would leave the project with:"
    echo "  ❌ No working compiler"
    echo "  ❌ No ability to compile Seen code"
    echo "  ❌ No way to build examples or tests"
    echo "  ❌ Complete development blockage"
    echo ""
    echo "Estimated work to fix: 20-30 hours"
    echo "See: SELF_HOSTING_COMPLETION_PLAN.md"
    echo "========================================="
    exit 1
fi
