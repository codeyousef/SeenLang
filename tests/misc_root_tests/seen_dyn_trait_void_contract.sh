#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-dyn-trait-void
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/dyn-trait-void.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=dyn-trait-void
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" lifecycle "$ROOT_DIR/tests/codegen/test_dyn_trait_void.seen"
    rg -Fq 'PASS: dynamic Void dispatch preserves arguments nonvoid returns and exactly-once cleanup' "$TMP_DIR/lifecycle-$profile.log"
done
rg --no-ignore -q 'call void .* ; dyn Lifecycle.add' "$TMP_DIR/lifecycle-fast-ir"
rg --no-ignore -q 'call void .* ; dyn Lifecycle.release' "$TMP_DIR/lifecycle-fast-ir"
echo 'PASS: dynamic Void calls and mixed return types in fast release UBSan and ASan'
