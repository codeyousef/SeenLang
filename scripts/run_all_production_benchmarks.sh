#!/bin/bash
# Compatibility wrapper for the maintained production benchmark runner.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/run_production_benchmarks.sh" "$@"
