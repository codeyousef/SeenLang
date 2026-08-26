#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: native dependency probe requires the verified hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
PROBE_ROOT="${SEEN_ARTIFACT_ROOT:?}/qwen-native-dependency"
mkdir -p "$PROBE_ROOT"
cp -R "$ROOT_DIR/tests/fixtures/qwen/native_dependency/." "$PROBE_ROOT"
mkdir -p "$PROBE_ROOT/native/lib"
cc -std=c11 -Wall -Wextra -Werror -fPIC -shared \
    "$PROBE_ROOT/native/qwen_probe.c" -o "$PROBE_ROOT/native/lib/libqwen_probe.so"
(
    cd "$PROBE_ROOT"
    "$SEEN_BIN" compile src/main.seen "$PROBE_ROOT/probe" --release \
        --lto=thin --target-cpu=x86-64 --no-cache --jobs 1 --opt-jobs 1
)
"$PROBE_ROOT/probe"
readelf -d "$PROBE_ROOT/probe" | grep -Fq "$PROBE_ROOT/native/lib"
echo "PASS: local native dependency, fixed-width pointer/struct ABI, ThinLTO, and rpath"
