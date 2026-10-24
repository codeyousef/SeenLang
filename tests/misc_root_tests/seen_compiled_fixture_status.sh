#!/usr/bin/env bash
# Exercise exact compiler/executable failures under conditional callers.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-compiled-fixture-status
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/compiled-fixture-status.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=compiled-fixture-status
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
probe="$ROOT_DIR/tests/fixtures/compiled-fixture-policy/exit_status.seen"
compile_fixture fast exit-status "$probe"
run_fixture fast exit-status pass
status=0
if run_fixture fast exit-status fail; then status=0; else status=$?; fi
[ "$status" = 23 ] || { echo "FAIL: conditional fixture execution returned $status instead of 23" >&2; exit 1; }
if rg -Fq 'PASS: fixture status probe' "$TMP_DIR/exit-status-fast.log"; then
    echo 'FAIL: failed execution log retained an earlier PASS' >&2; exit 1
fi
invalid="$ROOT_DIR/tests/fixtures/compiled-fixture-policy/compile_error_rejected.seen"
expected=0
if run_compiler compile "$invalid" "$TMP_DIR/expected-failure" --fast --no-cache --frozen > "$TMP_DIR/expected-compiler-failure.log" 2>&1; then
    echo 'FAIL: deliberately invalid source compiled' >&2; exit 1
else expected=$?; fi
case "$expected" in 0|124|125|126|137|143) echo "FAIL: invalid compiler failure baseline $expected" >&2; exit 1 ;; esac
status=0
if compile_case fast exit-status "$invalid" stale; then status=0; else status=$?; fi
[ "$status" = "$expected" ] || { echo "FAIL: conditional compile/run returned $status instead of compiler status $expected" >&2; exit 1; }
status=0
if run_fixture fast exit-status stale; then status=0; else status=$?; fi
[ "$status" = "$expected" ] || { echo "FAIL: ignored compiler failure admitted a stale artifact (status $status)" >&2; exit 1; }
if rg -Fq 'stale executable ran' "$TMP_DIR/00-active.log"; then
    echo 'FAIL: compiler failure executed an existing artifact' >&2; exit 1
fi
echo 'PASS: fixture failures preserve exact status and isolated logs under conditional callers'
