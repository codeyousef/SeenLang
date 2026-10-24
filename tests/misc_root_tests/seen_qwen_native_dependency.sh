#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
SEEN_BIN=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-qwen-native-dependency
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$SEEN_BIN" -- bash "$0" "$@"
fi
SEEN_BIN="${SEEN_CAPPED_REGRESSION_COMPILER:-$SEEN_BIN}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$SEEN_BIN"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
PROBE_ROOT="${SEEN_ARTIFACT_ROOT:?}/qwen-native-dependency"
mkdir -p "$PROBE_ROOT"
cp -R "$ROOT_DIR/tests/fixtures/qwen/native_dependency/." "$PROBE_ROOT"
mkdir -p "$PROBE_ROOT/native/lib"
cc -std=c11 -Wall -Wextra -Werror -fPIC -shared \
    "$PROBE_ROOT/native/qwen_probe.c" -o "$PROBE_ROOT/native/lib/libqwen_probe.so"
(
    cd "$PROBE_ROOT"
    bash "$ATTESTED_SEEN" "$SEEN_BIN" compile src/main.seen "$PROBE_ROOT/probe" --release \
        --lto=thin --target-cpu=x86-64 --no-cache
)
timeout --kill-after=5s 120s "$PROBE_ROOT/probe"
readelf -d "$PROBE_ROOT/probe" | grep -Fq "$PROBE_ROOT/native/lib"
echo "PASS: local native dependency, fixed-width pointer/struct ABI, ThinLTO, and rpath"
