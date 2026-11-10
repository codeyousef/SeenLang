#!/usr/bin/env bash
# Real native driver behavior; no Seen compiler or mock tool executes here.
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=toolchain-driver-aliases
if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_PLATFORM_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED_ENTRY" --platform "$SCOPE" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-platform-active "$SCOPE"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/toolchain-aliases.XXXXXX")"

timeout --foreground --kill-after=5s 30s ld.lld --version \
    >"$TMP_DIR/lld.version.log" 2>&1
grep -Fq 'LLD ' "$TMP_DIR/lld.version.log"
timeout --foreground --kill-after=5s 30s clang++ --version \
    >"$TMP_DIR/clang.version.log" 2>&1
timeout --foreground --kill-after=5s 60s clang++ -std=c++17 -O0 \
    -march=x86-64 -fuse-ld=lld \
    "$ROOT_DIR/tests/fixtures/test-002a/toolchain_alias.cpp" \
    -o "$TMP_DIR/cxx-alias" >"$TMP_DIR/cxx.compile.log" 2>&1
timeout --foreground --kill-after=5s 30s "$TMP_DIR/cxx-alias" \
    >"$TMP_DIR/cxx.run.log" 2>&1
grep -Fxq 'PASS: C++ driver alias links its language runtime' "$TMP_DIR/cxx.run.log"
echo 'PASS: bounded linker flavor and C++ driver aliases execute natively'
