#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_native_boundaries.py"
LEDGER="$ROOT_DIR/docs/architecture/native-boundaries.json"
INVALID="$ROOT_DIR/tests/fixtures/p0-arch-001/native-boundaries-invalid.json"

python3 "$CHECKER" "$LEDGER"
if python3 "$CHECKER" "$INVALID" >/dev/null 2>&1; then
    echo "FAIL: malformed native-boundary fixture was accepted" >&2
    exit 1
fi
echo "PASS: native-boundary ledger"
