#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-project-qualified-test-import
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-project-qualified-import.XXXXXX")"

cleanup() {
    local status=$?
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-project-qualified-import.*)
            if [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ]; then
                rm -rf -- "$TMP_DIR"
            else
                echo "ERROR: refusing to clean unsafe import regression path: $TMP_DIR" >&2
                status=1
            fi
            ;;
        *) status=1 ;;
    esac
    trap - EXIT
    exit "$status"
}
trap cleanup EXIT

run_logged() {
    local label=$1
    local log=$2
    shift 2
    local status=0

    "$@" >"$log" 2>&1 || status=$?
    if [ "$status" -ne 0 ]; then
        echo "FAIL: $label exited with status $status" >&2
        tail -c 32768 "$log" >&2 || true
        return "$status"
    fi
}

mkdir -p "$TMP_DIR/src/trainer" "$TMP_DIR/tests" "$TMP_DIR/target"
cat >"$TMP_DIR/Seen.toml" <<'TOML'
manifest-version = 1

[project]
name = "trainer"
version = "0.1.0"
language = "en"
edition = "2025"

[build]
entry = "src/main.seen"
targets = ["native"]
TOML
cat >"$TMP_DIR/src/main.seen" <<'SEEN'
fun main() r: Int { return 0 }
SEEN
cat >"$TMP_DIR/src/trainer/jsonl.seen" <<'SEEN'
pub fun tripletScore(a: String, p: String, n: String) r: Int {
    return a.length() + p.length() + n.length()
}
SEEN
cat >"$TMP_DIR/tests/test_jsonl.seen" <<'SEEN'
import trainer.jsonl.{tripletScore}

fun main() r: Int {
    if tripletScore("x", "yy", "zzz") == 6 { return 0 }
    return 1
}
SEEN

(
    cd "$TMP_DIR"
    run_logged "qualified-layout semantic check" "$TMP_DIR/check.log" \
        timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" check tests/test_jsonl.seen
    run_logged "qualified-layout compile" "$TMP_DIR/compile.log" \
        timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile tests/test_jsonl.seen \
        target/test-jsonl --fast --no-cache
)
if rg -n 'tests/trainer|(^|[[:space:]])find:|error\[E005\]' \
    "$TMP_DIR/check.log" "$TMP_DIR/compile.log"; then
    echo "FAIL: qualified layout retained the historical tests/trainer resolution error" >&2
    exit 1
fi
run_logged "qualified-layout executable" "$TMP_DIR/run.log" \
    timeout --foreground --kill-after=5s 30s "$TMP_DIR/target/test-jsonl"

mkdir -p "$TMP_DIR/direct/src" "$TMP_DIR/direct/tests" "$TMP_DIR/direct/target"
cp "$TMP_DIR/Seen.toml" "$TMP_DIR/direct/Seen.toml"
cp "$TMP_DIR/src/main.seen" "$TMP_DIR/direct/src/main.seen"
cp "$TMP_DIR/src/trainer/jsonl.seen" "$TMP_DIR/direct/src/jsonl.seen"
cp "$TMP_DIR/tests/test_jsonl.seen" "$TMP_DIR/direct/tests/test_jsonl.seen"
(
    cd "$TMP_DIR/direct"
    run_logged "direct-layout semantic check" "$TMP_DIR/direct-check.log" \
        timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" check tests/test_jsonl.seen
    run_logged "direct-layout compile" "$TMP_DIR/direct-compile.log" \
        timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile tests/test_jsonl.seen \
        target/test-jsonl --fast --no-cache
)
if rg -n 'tests/trainer|(^|[[:space:]])find:|error\[E005\]' \
    "$TMP_DIR/direct-check.log" "$TMP_DIR/direct-compile.log"; then
    echo "FAIL: direct layout retained the historical tests/trainer resolution error" >&2
    exit 1
fi
run_logged "direct-layout executable" "$TMP_DIR/direct-run.log" \
    timeout --foreground --kill-after=5s 30s \
    "$TMP_DIR/direct/target/test-jsonl"

echo "PASS: check and compile agree on project-qualified test imports"
