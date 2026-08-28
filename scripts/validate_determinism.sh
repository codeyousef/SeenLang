#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPILER="${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}"
TEST_FILE="${1:-$ROOT_DIR/tests/fixtures/core-004c/deterministic_ok.seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=core-004c-deterministic-check

fail() {
    echo "core.004c.invalid: $*" >&2
    exit 1
}

if [[ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi

COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
ARTIFACT_ROOT="${SEEN_ARTIFACT_ROOT:?}"
[[ -f "$TEST_FILE" && ! -L "$TEST_FILE" ]] || fail "unsafe or missing input"

WORK_DIR="$(mktemp -d "$ARTIFACT_ROOT/core-004c-check.XXXXXX")"
cleanup() {
    case "$WORK_DIR" in
        "$ARTIFACT_ROOT"/core-004c-check.*)
            [[ -d "$WORK_DIR" && ! -L "$WORK_DIR" ]] && rm -rf -- "$WORK_DIR"
            ;;
        *) fail "refusing to remove unexpected work directory" ;;
    esac
}
trap cleanup EXIT

for run in 1 2 3; do
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TEST_FILE" \
        --deterministic >"$WORK_DIR/run-$run.out" 2>&1 ||
        fail "deterministic check failed on run $run"
done

cmp -s "$WORK_DIR/run-1.out" "$WORK_DIR/run-2.out" ||
    fail "diagnostics differed between runs 1 and 2"
cmp -s "$WORK_DIR/run-1.out" "$WORK_DIR/run-3.out" ||
    fail "diagnostics differed between runs 1 and 3"

echo "PASS: CORE-004C deterministic CLI check"
