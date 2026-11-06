#!/usr/bin/env bash
# Slow discovery runs a bounded real software smoke, never the manual 24h case.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
RUNNER="$ROOT_DIR/scripts/run_async_stress_soak.sh"
reject_argument() {
    local status=0 output
    output=$(bash "$RUNNER" "$@" 2>&1) || status=$?
    [ "$status" -eq 2 ] || { echo "FAIL: invalid manual soak argument returned $status" >&2; exit 1; }
    case "$output" in
        *async.invalid:*|*async.limit:*) ;;
        *) echo 'FAIL: invalid manual soak argument lacked stable diagnostic' >&2; exit 1 ;;
    esac
}
for duration in 0 -1 86401 999999999999999999999999999999999999 1x ''; do
    reject_argument --seconds "$duration" --device-class software-reference
done
reject_argument --seconds
reject_argument --seconds 2 --device-class invalid-device
reject_argument --seconds 2 --device-class
# This builds the actual release/ThinLTO native workload once, exercises native
# invalid argv before I/O setup, then checks real >=2s completion, counters,
# /proc observations, exact child exit and process-group lifetime cleanup.
exec bash "$RUNNER" --seconds 2 --device-class software-reference
