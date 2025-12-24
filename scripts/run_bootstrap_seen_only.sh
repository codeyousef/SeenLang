#!/usr/bin/env bash
# S1 Validation: Run a 3-stage Seen-only bootstrap and a smoke test using the self-hosted compiler.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-0}"

echo "🎯 Running Seen-only bootstrap (Stage1→Stage3)"
./scripts/validate_d2_determinism.sh

echo "🚀 Using stage3.out to build a sample program (bootstrap_test/src/main.seen)"
./stage3.out compile bootstrap_test/src/main.seen bootstrap_test_out
./bootstrap_test_out

echo "✅ Building and running hello_cli via stage3.out"
./stage3.out compile examples/linux/hello_cli/main.seen hello_cli_out
./hello_cli_out

echo "🎉 Seen-only bootstrap and smoke tests complete"
