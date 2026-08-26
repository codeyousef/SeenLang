#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
exec bash "$ROOT_DIR/tests/misc_root_tests/seen_fs_contract.sh" "$@"
