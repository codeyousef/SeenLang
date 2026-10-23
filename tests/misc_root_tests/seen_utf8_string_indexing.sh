#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}"
RELEASE_TARGET_CPU="${SEEN_RELEASE_CPU_BASELINE:-x86-64}"
case "$RELEASE_TARGET_CPU" in
    x86-64|x86-64-v3) ;;
    *)
        echo "FAIL: unsupported tokenizer contract CPU baseline: $RELEASE_TARGET_CPU" >&2
        exit 2
        ;;
esac
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-utf8-string-indexing

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
COMPILER_PREFIX=(bash "${SEEN_ATTESTED_COMPILER_RUNNER:?}")
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-utf8-indexing.XXXXXX")"
case "$TMP_DIR" in
    "$SEEN_ARTIFACT_ROOT"/seen-utf8-indexing.*) ;;
    *) echo "Error: unsafe tokenizer artifact path: $TMP_DIR" >&2; exit 1 ;;
esac
FIXTURE_ARTIFACT_PREFIX=seen-utf8-indexing
FIXTURE_TARGET_CPU="$RELEASE_TARGET_CPU"
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
cleanup() {
    local status=$?
    if [ "$status" -ne 0 ] || [ -n "${SEEN_KEEP_TMP:-}" ]; then
        echo "Preserved UTF-8 tokenizer artifacts: $TMP_DIR (status $status)" >&2
        exit "$status"
    fi
    [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
        [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || exit 1
    rm -rf -- "$TMP_DIR"
}
trap cleanup EXIT

cd "$ROOT_DIR/packages/seen_tokenizers"
for profile in fast release undefined address; do
    compile_case "$profile" character-lifecycle \
        "$ROOT_DIR/tests/fixtures/test-002a/character_string_lifecycle.seen"
    compile_case "$profile" bpe-lifecycle "$PWD/tests/bpe_lifecycle.seen"
done
timeout --foreground --kill-after=10s 600s \
    "${COMPILER_PREFIX[@]}" "$COMPILER" compile tests/tokenizers_contract.seen \
    "$TMP_DIR/tokenizers-contract" --release --lto thin --no-cache \
    --no-fork --target-cpu "$RELEASE_TARGET_CPU" \
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
    --target-cpu "$RELEASE_TARGET_CPU" >"$TMP_DIR/production-compile.log" 2>&1 || {
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
