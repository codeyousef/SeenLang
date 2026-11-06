#!/usr/bin/env bash
# Opt-in source SDK construction; every entry obtains fresh aggregate admission.
set -euo pipefail
ROOT_DIR=$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)
fail() { echo "RESOURCE STOP: finite TSan SDK: $*" >&2; exit 126; }
if [ "${1:-}" = --inside ]; then
    shift
    [ "$#" = 2 ] && [ "$1" = --output ] || fail 'expected --output /absolute/new/path'
    bash "$ROOT_DIR/scripts/run_capped_regression.sh" --verify-platform-active seen-finite-tsan-sdk
    exec python3 "$ROOT_DIR/scripts/finite_tsan_sdk.py" build --output "$2"
fi
[ "$#" = 2 ] && [ "$1" = --output ] || fail 'expected --output /absolute/new/path'
[ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != 1 ] || fail 'fresh SDK scope required, not inherited admission'
[ "${SEEN_TSAN_SDK_MANIFEST+x}" != x ] || fail 'SDK construction must not inherit explicit SDK selection'
case "$2" in /*) ;; *) fail 'output must be absolute' ;; esac
exec env SEEN_MEMORY_GUARD_RSS_KB=2097152 SEEN_MEMORY_GUARD_CGROUP_STOP_KB=2097152 \
    SEEN_MAIN_VMEM_KB=2097152 SEEN_OPT_VMEM_KB=2097152 SEEN_MEMORY_LIMIT_BYTES=2147483648 \
    bash "$ROOT_DIR/scripts/run_capped_regression.sh" --platform seen-finite-tsan-sdk -- \
    bash "$ROOT_DIR/scripts/install_finite_tsan_sdk.sh" --inside --output "$2"
