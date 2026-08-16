#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-comptime-fail-closed
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
ARTIFACT_ROOT="$SEEN_ARTIFACT_ROOT"
TMP_DIR="$(mktemp -d "$ARTIFACT_ROOT/seen-comptime-fail-closed.XXXXXX")"
cleanup() {
    case "$TMP_DIR" in
        "$ARTIFACT_ROOT"/seen-comptime-fail-closed.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) echo "refusing to remove unexpected test path: $TMP_DIR" >&2; return 1 ;;
    esac
}
trap cleanup EXIT

run_check() {
    bash "$ATTESTED_SEEN" "$COMPILER" check "$1"
}

printf '%s\n' \
    'fun runtimeValue() r: Bool { return true }' \
    'fun main() {' \
    '    comptime assert(runtimeValue(), "must not lower at runtime")' \
    '}' >"$TMP_DIR/unsupported.seen"

if run_check "$TMP_DIR/unsupported.seen" >"$TMP_DIR/unsupported.log" 2>&1; then
    echo "unsupported comptime expression was accepted" >&2
    exit 1
fi
grep -Fq 'condition is not supported by the compile-time evaluator' \
    "$TMP_DIR/unsupported.log"

printf '%s\n' \
    'fun main() {' \
    '    comptime assert(false, "expected compile-time failure")' \
    '}' >"$TMP_DIR/false_assert.seen"

if run_check "$TMP_DIR/false_assert.seen" >"$TMP_DIR/false.log" 2>&1; then
    echo "false comptime assertion was accepted" >&2
    exit 1
fi
grep -Fq 'compile-time assertion failed' "$TMP_DIR/false.log"

echo "comptime fail-closed diagnostics passed"
