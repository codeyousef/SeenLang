#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
entry="$ROOT_DIR/scripts/safe_rebuild.sh"
valid_script="$ROOT_DIR/tests/misc_root_tests/seen_gpu_ptx_a.sh"
reject() {
    local status=0 diagnostic
    diagnostic=$(bash "$entry" "$@" 2>&1) || status=$?
    [ "$status" = 2 ] && [[ "$diagnostic" == *ERROR:* ]] || {
        printf 'FAIL: focused entry accepted invalid arguments (status %s): %s\n' "$status" "$diagnostic" >&2
        exit 1
    }
    if [[ "$diagnostic" == *'Entering verified aggregate'* ]]; then
        echo 'FAIL: invalid focused invocation reached build setup' >&2
        exit 1
    fi
}
reject --regression-script ''
reject --regression-script
reject --regression-script /etc/profile
reject --regression-script "$ROOT_DIR/../escape.sh"
reject --regression-script "$valid_script" --regression-script "$valid_script"
reject --regression-script "$valid_script" --clean-cache
reject --regression-script "$valid_script" --artifact-preflight
reject --regression-script "$valid_script" --tier full
echo 'PASS: focused regression entry rejects ambiguous and unsafe invocation before build setup'
