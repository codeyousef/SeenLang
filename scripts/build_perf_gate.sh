#!/usr/bin/env bash
# Compatibility wrapper for the canonical Seen performance gate.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/perf_gate.sh" --suite build "$@"
