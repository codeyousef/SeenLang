#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
HARD_SCOPE="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"
SCOPE=seen-utf8-string-indexing

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ] &&
   [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
COMPILER_PREFIX=()
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" = "1" ]; then
    bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
    COMPILER_PREFIX=(bash "${SEEN_ATTESTED_COMPILER_RUNNER:?}")
else
    # Required CI already owns a read-back-verified aggregate cgroup.  The
    # compile below is explicitly serial and no-fork, so retain its command
    # VMEM and timeout bounds without demanding an unrelated inherited
    # fork-serializer record from a prior rebuild process.
    bash "$HARD_SCOPE" --label "$SCOPE read-back" --verify-only -- >/dev/null
    [ "${SEEN_LOW_MEMORY:-0}" = "1" ] && [ "${SEEN_JOBS:-0}" = "1" ] &&
        [ "${SEEN_OPT_JOBS:-0}" = "1" ] || {
        echo "RESOURCE STOP: tokenizer regression requires serial low-memory settings" >&2
        exit 126
    }
    case "${SEEN_MAIN_VMEM_KB:-}" in
        ''|*[!0-9]*|0) echo "RESOURCE STOP: tokenizer regression VMEM cap is invalid" >&2; exit 126 ;;
    esac
    ulimit -S -v "$SEEN_MAIN_VMEM_KB" || {
        echo "RESOURCE STOP: tokenizer regression could not apply its VMEM cap" >&2
        exit 126
    }
fi
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-utf8-indexing.XXXXXX")"
case "$TMP_DIR" in
    "$SEEN_ARTIFACT_ROOT"/seen-utf8-indexing.*) ;;
    *) echo "Error: unsafe tokenizer artifact path: $TMP_DIR" >&2; exit 1 ;;
esac
cleanup() {
    rm -rf -- "$TMP_DIR"
}
trap cleanup EXIT

cd "$ROOT_DIR/packages/seen_tokenizers"
timeout --foreground --kill-after=10s 600s \
    "${COMPILER_PREFIX[@]}" "$COMPILER" compile tests/tokenizers_contract.seen \
    "$TMP_DIR/tokenizers-contract" --release --lto thin --no-cache \
    --no-fork --jobs 1 --opt-jobs 1 >"$TMP_DIR/compile.log" 2>&1 || {
    tail -c 32768 "$TMP_DIR/compile.log" >&2
    exit 1
}
cd "$ROOT_DIR"
timeout --foreground --kill-after=5s 60s "$TMP_DIR/tokenizers-contract"
echo "PASS: FEL-589 UTF-8 byte/codepoint indexing contract"
