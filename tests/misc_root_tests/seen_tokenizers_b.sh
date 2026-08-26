#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bash "$ROOT_DIR/tests/misc_root_tests/seen_utf8_string_indexing.sh"
echo "PASS: SEEN-LIB-004B byte BPE and special-token handling"
