#!/bin/bash
# E2E Multi-Language Test Runner for Seen Compiler
# Tests all keywords and stdlib across 6 languages (en, ar, es, ja, ru, zh)

set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
BUILD_CMD="compile"
LANGS="en ar es ja ru zh"
PASS=0
FAIL=0
SKIP=0
ERRORS=""

if [ ! -x "$COMPILER" ]; then
    echo "ERROR: Compiler not found at $COMPILER"
    exit 2
fi

echo "=== Seen E2E Multi-Language Test Suite ==="
echo "Compiler: $COMPILER"
echo "Languages: $LANGS"
echo ""

for lang in $LANGS; do
    echo "--- Language: $lang ---"
    for test_file in "$SCRIPT_DIR/$lang"/*.seen; do
        [ -f "$test_file" ] || continue
        name=$(basename "$test_file" .seen)
        binary="/tmp/seen_e2e_${lang}_${name}"
        rm -rf "$ROOT_DIR/.seen_cache/" 2>/dev/null
        rm -rf /tmp/seen_ir_cache 2>/dev/null
        rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll 2>/dev/null

        # Check if compile-only (header comment contains "COMPILE-ONLY")
        if head -3 "$test_file" | grep -q "COMPILE-ONLY"; then
            if timeout 120 "$COMPILER" $BUILD_CMD "$test_file" "$binary" --fast --language "$lang" >/dev/null 2>&1; then
                echo "  PASS [compile] $name"
                PASS=$((PASS+1))
            else
                echo "  FAIL [compile] $name"
                FAIL=$((FAIL+1))
                ERRORS="$ERRORS\n  FAIL [compile] $lang/$name"
            fi
        else
            # Runtime test: compile then run
            if timeout 120 "$COMPILER" $BUILD_CMD "$test_file" "$binary" --fast --language "$lang" >/dev/null 2>&1; then
                if timeout 30 "$binary" >/dev/null 2>&1; then
                    echo "  PASS [runtime] $name"
                    PASS=$((PASS+1))
                else
                    echo "  FAIL [runtime] $name"
                    FAIL=$((FAIL+1))
                    ERRORS="$ERRORS\n  FAIL [runtime] $lang/$name"
                fi
            else
                echo "  FAIL [compile] $name"
                FAIL=$((FAIL+1))
                ERRORS="$ERRORS\n  FAIL [compile] $lang/$name"
            fi
        fi
        rm -f "$binary"
    done
    echo ""
done

echo "==========================================="
echo "TOTAL: $PASS passed, $FAIL failed, $SKIP skipped"
echo "==========================================="

if [ $FAIL -gt 0 ]; then
    echo ""
    echo "Failed tests:"
    echo -e "$ERRORS"
    exit 1
fi

exit 0
