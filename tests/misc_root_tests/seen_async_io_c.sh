#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-io-c
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/completion-io-c.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=completion-io-c
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" contract "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/iocp_contract.seen"
    compile_case "$profile" native "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/iocp_windows.seen" "$TMP_DIR/native-$profile.data"
    rg -Fq '"schema":"seen-io-iocp-v1"' "$TMP_DIR/native-$profile.log"
    if [ "${SEEN_REQUIRE_IOCP:-0}" = 1 ]; then
        rg -Fq '"classification":"verified"' "$TMP_DIR/native-$profile.log"
    fi
done
compile_case fast fuzz "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/iocp_fuzz.seen"
# Win64 passes 16-byte aggregates by reference and returns them through sret;
# LLVM's split-scalar lowering crashed every Windows binary at the first call.
WIN64_IR="$TMP_DIR/win64-ir"
run_compiler compile "$ROOT_DIR/tests/cross_platform_asynchronous_i_o/iocp_windows.seen" \
    "$TMP_DIR/win64" --target windows-x86_64 --no-cache --emit-module-ir-dir "$WIN64_IR" \
    --stop-after-ir > "$TMP_DIR/win64-ir.log" 2>&1
rg -Fq 'declare void @seen_cstr_to_str(ptr sret(%SeenString) align 8, ptr)' "$WIN64_IR"/*.ll
rg -Fq 'declare void @println(ptr)' "$WIN64_IR"/*.ll
if rg -q '^declare %SeenString ' "$WIN64_IR"/*.ll; then
    echo 'FAIL: Win64 external aggregate signature left unlowered' >&2
    exit 1
fi
for ir in "$WIN64_IR"/*.ll; do opt -passes=verify -disable-output "$ir"; done
echo 'PASS: overlapped completion contracts; platform classification retained in fixture evidence'
