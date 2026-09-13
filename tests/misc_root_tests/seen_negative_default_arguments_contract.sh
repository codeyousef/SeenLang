#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-negative-default-arguments
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE="$ROOT_DIR/tests/fixtures/fel-1578/negative_defaults.seen"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/negative-defaults.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/negative-defaults.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved negative-default artifacts: $WORK_DIR" >&2
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
    output="$WORK_DIR/negative-defaults-$profile"
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
    find "$ir_dir" -maxdepth 1 -type f -name '*.ll' -print0 |
        sort -z | xargs -0 python3 "$ROOT_DIR/scripts/verify_ir_call_shapes.py"
done

find "$WORK_DIR/ir-release" -maxdepth 1 -type f -name '*.ll' -print0 |
    sort -z | xargs -0 cat > "$WORK_DIR/release.ll"

for expected in \
    'call i8 @sameInt8(i8 -1)' \
    'call i16 @sameInt16(i16 -2)' \
    'call i32 @sameInt32(i32 -3)' \
    'call i64 @sameInt64(i64 -4)' \
    'call i64 @sameInt(i64 -5)' \
    'call i8 @importedInt8(i8 -6)' \
    'call i16 @importedInt16(i16 -7)' \
    'call i32 @importedInt32(i32 -8)' \
    'call i64 @importedInt64(i64 -9)' \
    'call i64 @importedInt(i64 -10)'
do
    grep -Fq "$expected" "$WORK_DIR/release.ll" || {
        echo "FAIL: release IR omitted exact call shape: $expected" >&2
        exit 1
    }
done

if grep -Eq 'call i(8|16|32|64) @[A-Za-z0-9_]+\(i64 -\)' \
    "$WORK_DIR/release.ll"; then
    echo "FAIL: release IR retained a bare negative default operand" >&2
    exit 1
fi

grep -Fq 'captureDefaultParameterValue()' \
    "$ROOT_DIR/compiler_seen/src/parser/real_parser.seen"
grep -Fq 'getFixedWidthScalarLlvmTypeImpl(dfType)' \
    "$ROOT_DIR/compiler_seen/src/codegen/ir_call_args.seen"

echo "PASS: FEL-1578 negative fixed-width default argument contract"
