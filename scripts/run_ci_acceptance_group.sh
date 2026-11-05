#!/usr/bin/env bash
# Re-establish live containment for one complete same-candidate acceptance group.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
CANDIDATE="$ROOT_DIR/target/seen-build/bootstrap-candidate"
CHECKER="$ROOT_DIR/scripts/ci_bootstrap_candidate.py"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
fail() { echo "ci-acceptance: core.001b.invalid: $*" >&2; exit 126; }

# This is only re-entry routing, never a scope or compiler attestation.
if [ "${SEEN_CI_ACCEPTANCE_INNER:-0}" = 1 ]; then
    [ "$#" -eq 0 ] || fail "inner acceptance does not accept arguments"
    GROUP="${SEEN_CI_ACCEPTANCE_GROUP:-}"
else
    [ "$#" -eq 1 ] || fail "one complete acceptance group is required"
    GROUP=$1
fi
case "$GROUP" in
    core|io-simd|gpu-emitter|gpu-runtime|async) ;;
    *) fail "unknown acceptance group: $GROUP" ;;
esac
[ "${SEEN_CI_CONTAINMENT_IN_SCOPE:-0}" = 1 ] ||
    fail "acceptance must enter through the contained CI command"
python3 "$CHECKER" verify --root "$ROOT_DIR" --candidate-dir "$CANDIDATE"

if [ "${SEEN_CI_ACCEPTANCE_INNER:-0}" != 1 ]; then
    export SEEN_STAGE_BUILDER="$CANDIDATE/seen"
    export SEEN_PACKAGE_CLIENT="$CANDIDATE/seen-pkg"
    export SEEN_STAGE_BUILDER_PACKAGE_CLIENT="$CANDIDATE/seen-pkg"
    export SEEN_STAGE_BUILDER_ONLY=1 SEEN_TRACE_BUILD=1 SEEN_KEEP_TMP=1
    export SEEN_CI_ACCEPTANCE_GROUP="$GROUP" SEEN_CI_ACCEPTANCE_INNER=1
    exec "$ROOT_DIR/scripts/safe_rebuild.sh" --regression-script "$0"
fi

# The canonical runner independently verifies the real scope and its freshly
# bound serializer. A failed native case exits before any receipt is published.
"$ROOT_DIR/scripts/seen_stage1_acceptance.sh" --tier verify \
    --compiler "$CANDIDATE/seen" --group "$GROUP"
python3 "$CHECKER" record-group --root "$ROOT_DIR" --candidate-dir "$CANDIDATE" \
    --group "$GROUP" \
    --output "$ROOT_DIR/target/seen-build/acceptance-receipts/$GROUP.json"
echo "PASS: complete acceptance group $GROUP; candidate remains unpublished"
