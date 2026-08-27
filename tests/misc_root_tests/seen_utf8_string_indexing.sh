#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
RELEASE_TARGET_CPU="${SEEN_RELEASE_CPU_BASELINE:-x86-64}"
case "$RELEASE_TARGET_CPU" in
    x86-64|x86-64-v3) ;;
    *)
        echo "FAIL: unsupported tokenizer contract CPU baseline: $RELEASE_TARGET_CPU" >&2
        exit 2
        ;;
esac
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
    --no-fork --target-cpu "$RELEASE_TARGET_CPU" --jobs 1 --opt-jobs 1 \
    >"$TMP_DIR/compile.log" 2>&1 || {
    tail -c 32768 "$TMP_DIR/compile.log" >&2
    exit 1
}
cd "$ROOT_DIR"
bash "$ROOT_DIR/scripts/check_x86_executable_baseline.sh" \
    "$RELEASE_TARGET_CPU" "$TMP_DIR/tokenizers-contract"
timeout --foreground --kill-after=5s 60s "$TMP_DIR/tokenizers-contract"

awk 'BEGIN {
    printf "{\"model\":{\"type\":\"BPE\",\"vocab\":{";
    for (i = 0; i < 248044; i++) {
        if (i > 0) printf ","
        if (i % 3 == 0) printf "\"token_%d_العربية\":%d", i, i
        else if (i % 3 == 1) printf "\"token_%d_中文\":%d", i, i
        else printf "\"token_%d_🙂\":%d", i, i
    }
    printf "},\"merges\":[]},\"added_tokens\":[]}\n"
}' > "$TMP_DIR/production-tokenizer.json"

cd "$ROOT_DIR/packages/seen_tokenizers"
timeout --foreground --kill-after=10s 900s \
    "${COMPILER_PREFIX[@]}" "$COMPILER" compile \
    tests/production_vocab_contract.seen "$TMP_DIR/production-vocab-contract" \
    --release --lto thin --no-cache --no-fork \
    --target-cpu "$RELEASE_TARGET_CPU" \
    --jobs 1 --opt-jobs 1 >"$TMP_DIR/production-compile.log" 2>&1 || {
    tail -c 32768 "$TMP_DIR/production-compile.log" >&2
    exit 1
}
cd "$ROOT_DIR"
bash "$ROOT_DIR/scripts/check_x86_executable_baseline.sh" \
    "$RELEASE_TARGET_CPU" "$TMP_DIR/production-vocab-contract"
timeout --foreground --kill-after=10s 900s \
    bash -c 'ulimit -S -s 8192; exec "$1" "$2"' _ \
    "$TMP_DIR/production-vocab-contract" \
    "$TMP_DIR/production-tokenizer.json"
echo "PASS: FEL-589 UTF-8 byte/codepoint indexing contract"
