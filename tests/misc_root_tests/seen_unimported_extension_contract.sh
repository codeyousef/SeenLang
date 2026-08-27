#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
RUNNER="$ROOT_DIR/scripts/run_with_project_artifacts.sh"
INVALID="$ROOT_DIR/tests/fixtures/fel-1535/unimported_extension.seen"
VALID="$ROOT_DIR/tests/fixtures/fel-1535/imported_extension.seen"
mkdir -p "$ROOT_DIR/.seen/agent-tools/fel-1535"
WORK_DIR=$(mktemp -d "$ROOT_DIR/.seen/agent-tools/fel-1535/run.XXXXXX")
cleanup() {
    status=$?
    if [ "$status" -eq 0 ]; then rm -rf "$WORK_DIR"
    else echo "Preserved failed FEL-1535 artifacts: $WORK_DIR" >&2; fi
}
trap cleanup EXIT

run_seen() {
    "$RUNNER" fel-1535 --keep-on-failure -- "$SEEN_BIN" "$@"
}

if run_seen check "$INVALID" >"$WORK_DIR/check-invalid.log" 2>&1; then
    echo "FAIL: unimported extension-style call passed check" >&2
    exit 1
fi
grep -Fq 'E009' "$WORK_DIR/check-invalid.log"
grep -Fq 'unresolved function `lines`' "$WORK_DIR/check-invalid.log"

if run_seen compile "$INVALID" "$WORK_DIR/invalid" \
    >"$WORK_DIR/compile-invalid.log" 2>&1; then
    echo "FAIL: unimported extension-style call compiled" >&2
    exit 1
fi
grep -Fq 'unresolved function `lines`' "$WORK_DIR/compile-invalid.log"
test ! -e "$WORK_DIR/invalid"

run_seen check "$VALID" >"$WORK_DIR/check-valid.log" 2>&1
run_seen compile "$VALID" "$WORK_DIR/valid-fast" \
    >"$WORK_DIR/compile-fast.log" 2>&1
"$WORK_DIR/valid-fast"
run_seen compile "$VALID" "$WORK_DIR/valid-release" --release --lto thin --no-fork \
    >"$WORK_DIR/compile-release.log" 2>&1
"$WORK_DIR/valid-release"

if rg -q '(^|[[:space:]])(int3|llvm\.trap)([[:space:](]|$)' "$WORK_DIR"; then
    echo "FAIL: extension-call contract emitted a trap" >&2
    exit 1
fi

echo "PASS: FEL-1535 extension-style import contract"
