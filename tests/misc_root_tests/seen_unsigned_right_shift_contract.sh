#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-unsigned-right-shift
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE="$ROOT_DIR/tests/fixtures/fel-1577/unsigned_right_shift.seen"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/unsigned-right-shift.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/unsigned-right-shift.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] && rm -rf -- "$WORK_DIR"
            else
                echo "Preserved unsigned-shift artifacts: $WORK_DIR" >&2
            fi
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT

run_compiler() {
    bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

for profile in fast release; do
    output="$WORK_DIR/unsigned-right-shift-$profile"
    ir_dir="$WORK_DIR/ir-$profile"
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1
        --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$FIXTURE"
    run_compiler compile "$FIXTURE" "$output" "${flags[@]}"
    timeout --foreground --kill-after=5s 60s "$output"
done

find "$WORK_DIR/ir-release" -maxdepth 1 -type f -name '*.ll' -print0 |
    sort -z | xargs -0 cat > "$WORK_DIR/release.ll"
grep -Eq 'lshr i64 .*' "$WORK_DIR/release.ll"
grep -Eq 'ashr i64 .*' "$WORK_DIR/release.ll"
if grep -Eq 'ashr i64 .*8000000000000000' "$WORK_DIR/release.ll"; then
    echo "FAIL: unsigned top-bit value used arithmetic right shift" >&2
    exit 1
fi
grep -Eq 'and i64 .*63' "$WORK_DIR/release.ll"

echo "PASS: FEL-1577 unsigned right-shift contract"
