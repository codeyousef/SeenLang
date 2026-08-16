#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
FIXTURE="$ROOT_DIR/tests/codegen/test_parallel_for_range_regression.seen"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-parallel-for-codegen
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
ARTIFACT_ROOT="$SEEN_ARTIFACT_ROOT"
TMP_DIR="$(mktemp -d "$ARTIFACT_ROOT/seen-pfor-codegen.XXXXXX")"
cleanup() {
    case "$TMP_DIR" in
        "$ARTIFACT_ROOT"/seen-pfor-codegen.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) echo "refusing to remove unexpected test path: $TMP_DIR" >&2; return 1 ;;
    esac
}
trap cleanup EXIT

run_compiler() {
    bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

run_compiler check "$FIXTURE" >/dev/null
run_compiler compile "$FIXTURE" "$TMP_DIR/program" \
    --no-cache \
    --emit-module-ir-dir "$TMP_DIR/ir" --stop-after-ir >/dev/null

IR_FILE="$(find "$TMP_DIR/ir" -maxdepth 1 -name '*.ll' -type f -print -quit)"
test -n "$IR_FILE"
grep -Eq 'call void @seen_parallel_for\(i64 2, i64 5, ptr @__seen_pfor_body_[0-9]+, i64 0\)' "$IR_FILE"
grep -Eq 'define internal i64 @__seen_pfor_body_[0-9]+\(i64 %0\)' "$IR_FILE"
grep -Fq '%pfor.index.addr = alloca i64' "$IR_FILE"
grep -Fq 'store i64 %0, ptr %pfor.index.addr' "$IR_FILE"
grep -Fq 'load i64, ptr %pfor.index.addr' "$IR_FILE"

cat >"$TMP_DIR/not_a_range.seen" <<'EOF'
fun main() {
    parallel_for i in 5 {
        println(i)
    }
}
EOF

if run_compiler check "$TMP_DIR/not_a_range.seen" \
    >"$TMP_DIR/invalid.log" 2>&1; then
    echo "parallel_for accepted a non-range expression" >&2
    exit 1
fi
grep -Fq 'parallel_for requires an exclusive start..end range' \
    "$TMP_DIR/invalid.log"

echo "parallel_for parser and codegen regression tests passed"
