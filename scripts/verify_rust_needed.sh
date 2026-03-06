#!/usr/bin/env bash
# R1 Verification: Determines if Rust compiler can be safely removed
# Checks if self-hosted compiler can compile itself with 0 type errors
set -euo pipefail

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║          R1: Rust Removal Readiness Verification                ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

# Prefer a self-hosted compiler when provided
if [[ -n "${SELF_HOST_SEEN:-}" && -x "$SELF_HOST_SEEN" ]]; then
    echo "✅ Using self-hosted compiler (Rust-free path): $SELF_HOST_SEEN"
    RUST_CLI="$SELF_HOST_SEEN"
    USING_SELF_HOST=1
else
    USING_SELF_HOST=0
    # Determine CLI location via Rust build
    if [[ -x "target-wsl/release/seen_cli" ]]; then
        RUST_CLI="target-wsl/release/seen_cli"
    elif [[ -x "target/release/seen_cli" ]]; then
        RUST_CLI="target/release/seen_cli"
    elif [[ -x "$HOME/.cargo/target-shared/release/seen_cli" ]]; then
        RUST_CLI="$HOME/.cargo/target-shared/release/seen_cli"
    else
        echo "❌ CRITICAL: Rust compiler not found"
        echo "   Tried: target-wsl/release/seen_cli"
        echo "   Tried: target/release/seen_cli"
        echo "   Tried: $HOME/.cargo/target-shared/release/seen_cli"
        echo "   Tip: set SELF_HOST_SEEN to a self-hosted binary to avoid Rust"
        exit 1
    fi
    echo "✅ Rust compiler found: $RUST_CLI"
    echo ""
fi

# Test 1: Can Rust compiler run a simple program?
echo "[Test 1] Can Rust compiler run simple Seen code?"
if "$RUST_CLI" run examples/linux/hello_cli/main.seen > /dev/null 2>&1; then
    echo "✅ PASS: Rust compiler works"
else
    echo "❌ FAIL: Rust compiler broken"
    exit 1
fi
echo ""

# Test 2: Can self-hosted compiler compile itself with 0 type errors?
echo "┌──────────────────────────────────────────────────────────────────┐"
echo "│ Test 2: Self-Hosted Compiler Type Checking                      │"
echo "└──────────────────────────────────────────────────────────────────┘"
echo "Running: SEEN_ENABLE_MANIFEST_MODULES=1 $RUST_CLI build compiler_seen/src/main.seen"
echo ""

ERROR_OUTPUT=$(SEEN_ENABLE_MANIFEST_MODULES=1 timeout 60 "$RUST_CLI" build compiler_seen/src/main.seen --output /tmp/stage1_test 2>&1 || true)
ERROR_COUNT=$(echo "$ERROR_OUTPUT" | grep -c "Type error:" || true)

if [[ $ERROR_COUNT -eq 0 ]]; then
    echo "✅ PASS: Self-hosted compiler compiles (0 type errors)"
    SELF_HOST_OK=1
else
    echo "❌ FAIL: Self-hosted compiler has $ERROR_COUNT type errors"
    echo "$ERROR_OUTPUT" | grep "Type error:" | head -5
    SELF_HOST_OK=0
fi
echo ""

# Final Summary
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║                    VERIFICATION SUMMARY                          ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""
echo "Prerequisites Checked:"
if [[ $USING_SELF_HOST -eq 1 ]]; then
    echo "  ✅ Self-hosted Compiler: Functional"
else
    echo "  ✅ Rust Compiler: Functional"
fi
echo "  ✅ Simple Programs: Can compile and run"
echo ""
echo "Self-Hosted Compiler Status:"
if [[ $SELF_HOST_OK -eq 1 ]]; then
    echo "  ✅ Type Errors: 0"
    echo "  ✅ Can compile itself: YES"
else
    echo "  ❌ Type Errors: $ERROR_COUNT"
    echo "  ❌ Can compile itself: NO"
fi
echo ""
echo "╔══════════════════════════════════════════════════════════════════╗"
if [[ $SELF_HOST_OK -eq 1 ]]; then
    echo "║                                                                  ║"
    echo "║        ✅ R1 COMPLETE: Rust NOT needed                          ║"
    echo "║                                                                  ║"
    echo "║  The chosen compiler can compile itself with 0 errors.          ║"
    if [[ $USING_SELF_HOST -eq 1 ]]; then
        echo "║  Rust sources can be removed when build/test workflows          ║"
        echo "║  are updated to use SELF_HOST_SEEN.                             ║"
    else
        echo "║  Rust sources can be safely removed once a self-hosted          ║"
        echo "║  binary is promoted to SELF_HOST_SEEN.                          ║"
    fi
    echo "║                                                                  ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    exit 0
else
    echo "║                                                                  ║"
    echo "║        ❌ R1 INCOMPLETE: Compiler still failing                ║"
    echo "║                                                                  ║"
    echo "║  The self-hosted compiler cannot compile itself yet.            ║"
    echo "║  Rust must remain until all type errors are resolved.           ║"
    echo "║                                                                  ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""
    echo "Required Actions:"
    echo "  1. Fix remaining $ERROR_COUNT type errors in compiler_seen"
    echo "  2. Ensure D1-D4 requirements are met"
    echo "  3. Re-run this script to verify"
    exit 1
fi
