#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
FIXTURE="$ROOT_DIR/tests/codegen/test_parallel_for_range_regression.seen"
MUTABLE_CAPTURE_FIXTURE="$ROOT_DIR/tests/fixtures/sync/parallel-capture-mutable.seen"
UNSHAREABLE_CAPTURE_FIXTURE="$ROOT_DIR/tests/fixtures/sync/parallel-capture-unshareable.seen"
BOUND_VALID_FIXTURE="$ROOT_DIR/tests/fixtures/sync/share-generic-bound-valid.seen"
BOUND_INVALID_FIXTURE="$ROOT_DIR/tests/fixtures/sync/share-generic-bound-invalid.seen"
LEGACY_SYNC_FIXTURE="$ROOT_DIR/tests/fixtures/sync/legacy-sync-invalid.seen"
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
grep -Eq 'call i64 @seen_parallel_for_v2\(i64 2, i64 5, ptr @__seen_pfor_body_[0-9]+, ptr %[0-9]+, ptr null, i64 4\)' "$IR_FILE"
grep -Eq 'define internal i64 @__seen_pfor_body_[0-9]+\(i64 %0, ptr %__seen_pfor_env\)' "$IR_FILE"
grep -Fq '%pfor.index.addr = alloca i64' "$IR_FILE"
grep -Fq 'store i64 %0, ptr %pfor.index.addr' "$IR_FILE"
grep -Fq 'load i64, ptr %pfor.index.addr' "$IR_FILE"
grep -Fq '= alloca {i64}' "$IR_FILE"
grep -Fq 'getelementptr inbounds {i64}, ptr %__seen_pfor_env' "$IR_FILE"
# Captures are parent-owned Share borrows. The worker binds the environment
# field directly and must not synthesize per-iteration ownership/drop scratch.
if sed -n '/define internal i64 @__seen_pfor_body_/,/^}/p' "$IR_FILE" |
        grep -Eq '= alloca \{i64\}|seen_(drop|release|free)'; then
    echo "parallel_for worker duplicated or dropped a borrowed capture" >&2
    exit 1
fi
grep -Fq 'declare i64 @seen_parallel_for_v2(i64, i64, ptr, ptr, ptr, i64)' "$IR_FILE"
grep -Fq 'call void @seen_parallel_for_fail_closed' "$IR_FILE"
if grep -Eq 'call void @seen_parallel_for\(' "$IR_FILE"; then
    echo "parallel_for lowering used the legacy capture-free ABI" >&2
    exit 1
fi

run_compiler compile "$FIXTURE" "$TMP_DIR/program-runtime" \
    --fast --no-cache >/dev/null
"$TMP_DIR/program-runtime" >"$TMP_DIR/program.out"
LC_ALL=C sort -n "$TMP_DIR/program.out" >"$TMP_DIR/program.sorted"
printf '9\n10\n11\n' >"$TMP_DIR/program.expected"
cmp -s "$TMP_DIR/program.expected" "$TMP_DIR/program.sorted"

run_compiler check "$BOUND_VALID_FIXTURE" >/dev/null

expect_check_failure() {
    local fixture="$1"
    local code="$2"
    local log="$3"
    if run_compiler check "$fixture" >"$log" 2>&1; then
        echo "expected compiler rejection for $fixture" >&2
        exit 1
    fi
    grep -Fq "$code" "$log"
}

expect_check_failure "$MUTABLE_CAPTURE_FIXTURE" E_CAPTURE_MUTABLE \
    "$TMP_DIR/mutable-capture.log"
expect_check_failure "$UNSHAREABLE_CAPTURE_FIXTURE" E_CAPTURE_SHARE \
    "$TMP_DIR/unshareable-capture.log"
expect_check_failure "$BOUND_INVALID_FIXTURE" E_TYPE_BOUND \
    "$TMP_DIR/generic-bound.log"
expect_check_failure "$LEGACY_SYNC_FIXTURE" E_PROPERTY_LEGACY \
    "$TMP_DIR/legacy-sync.log"

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
