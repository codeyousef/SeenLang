#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
cd -- "$ROOT_DIR"

python3 -m py_compile \
    scripts/check_native_boundaries.py \
    scripts/check_native_inventory.py \
    scripts/check_ci_workflows.py
python3 scripts/check_ci_workflows.py >/dev/null
tests/misc_root_tests/seen_native_boundaries_ledger.sh
tests/misc_root_tests/seen_native_inventory_gate.sh
tests/misc_root_tests/seen_ci_workflow_contract.sh
tests/misc_root_tests/seen_ci_required_wiring.sh
git diff --check

echo "PASS: required CI gates"
