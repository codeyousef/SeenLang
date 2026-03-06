#!/bin/bash
# seen_ir_verify.sh — Two-tier LLVM IR validation for the Seen compiler
#
# Tier 1: llvm-as structural verification (catches malformed IR)
# Tier 2: seen_ir_lint semantic checks (catches type mismatches, missing decls)
#
# Usage: seen_ir_verify.sh /tmp/seen_module_*.ll
# Exit code: 0 = clean, non-zero = errors found

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LINT="${SCRIPT_DIR}/seen_ir_lint"
ERRORS=0

if [ $# -eq 0 ]; then
    echo "Usage: $0 file1.ll [file2.ll ...]"
    exit 1
fi

# Build lint tool if not present
if [ ! -x "$LINT" ]; then
    if [ -f "${SCRIPT_DIR}/seen_ir_lint.c" ]; then
        echo "Building seen_ir_lint..."
        cc -O2 -Wall "${SCRIPT_DIR}/seen_ir_lint.c" -o "$LINT"
    else
        echo "WARNING: seen_ir_lint not found, running Tier 1 only"
        LINT=""
    fi
fi

# Tier 1: llvm-as structural verification
for ll in "$@"; do
    if [ ! -f "$ll" ]; then
        continue
    fi
    ERRFILE=$(mktemp /tmp/seen_verify_XXXXXX.err)
    if ! llvm-as "$ll" -o /dev/null 2>"$ERRFILE"; then
        echo "STRUCTURAL ERROR in $ll:"
        cat "$ERRFILE"
        ERRORS=$((ERRORS + 1))
    fi
    rm -f "$ERRFILE"
done

if [ $ERRORS -gt 0 ]; then
    echo ""
    echo "Tier 1 (llvm-as): $ERRORS file(s) with structural errors"
    exit 1
fi

# Tier 2: semantic lint
if [ -n "$LINT" ]; then
    if ! "$LINT" "$@"; then
        ERRORS=$((ERRORS + 1))
    fi
fi

exit $ERRORS
