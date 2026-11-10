#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ir-d-input-ownership
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ir-d-input-ownership.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ir-d-input-ownership
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" allocation-owner "$ROOT_DIR/tests/typed_gpu_compiler/allocation_owner_contract.seen"
    rg -Fq 'PASS: Seen allocation owner restores all four growing hash-map storage budgets' "$TMP_DIR/allocation-owner-$profile.log"
    compile_case "$profile" frontend-owner "$ROOT_DIR/tests/typed_gpu_compiler/frontend_ownership_contract.seen"
    rg -Fq 'PASS: owned GPU frontend success rejection scratch restoration and exact cleanup' "$TMP_DIR/frontend-owner-$profile.log"
    compile_case "$profile" quotes "$ROOT_DIR/tests/compiler_regressions/escaped_literal_quotes.seen"
    rg -Fq 'PASS: decoded string literals preserve meaningful quote bytes' "$TMP_DIR/quotes-$profile.log"
    compile_case "$profile" bytes "$ROOT_DIR/tests/typed_gpu_compiler/ir_bytes_contract.seen"
    rg -Fq 'PASS: GPU-001D byte text round trip all octets empty rejection and ownership' "$TMP_DIR/bytes-$profile.log"
done
echo 'PASS: GPU input ownership and byte/text contracts in all four profiles'
