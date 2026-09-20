#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-default-constant-arguments
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURES="$ROOT_DIR/tests/fixtures/default-constant-arguments"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/default-constant-arguments.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/default-constant-arguments.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved default-constant artifacts: $WORK_DIR" >&2
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

for fixture in same_module cross_module stdlib_bytes; do
    run_compiler check "$FIXTURES/$fixture.seen"
    for profile in fast release; do
        output="$WORK_DIR/$fixture-$profile"
        ir_dir="$WORK_DIR/$fixture-$profile-ir"
        flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1
            --emit-module-ir-dir "$ir_dir")
        if [ "$profile" = release ]; then
            flags+=(--release --lto=thin)
        else
            flags+=(--fast)
        fi
        run_compiler compile "$FIXTURES/$fixture.seen" "$output" "${flags[@]}"
        actual="$(timeout --foreground --kill-after=5s 60s "$output")"
        case "$fixture" in
            same_module) test "$actual" = 2 ;;
            cross_module) test "$actual" = 13 ;;
            stdlib_bytes) test "$actual" = 32 ;;
        esac
        if [ "$fixture" != stdlib_bytes ]; then
            find "$ir_dir" -maxdepth 1 -type f -name '*.ll' -print0 |
                sort -z | xargs -0 \
                python3 "$ROOT_DIR/scripts/verify_ir_call_shapes.py"
        fi
        if rg --no-ignore -q \
            '(i(1|8|16|32|64)|float|double) (EXIT_CODE|SMALL_|MEDIUM_|WIDE_|LARGE_|ENABLED|ENUM_LIKE_MODE|RATIO|IMPORTED_)' \
            "$ir_dir"; then
            echo "FAIL: unresolved source constant reached $fixture $profile IR" >&2
            exit 1
        fi
    done
done

release_ir="$WORK_DIR/same_module-release-ir"
for expected in \
    'call i64 @chooseExit(i64 2)' \
    'call i8 @chooseI8(i8 3)' \
    'call i8 @chooseU8(i8 4)' \
    'call i16 @chooseI16(i16 5)' \
    'call i16 @chooseU16(i16 6)' \
    'call i32 @chooseI32(i32 7)' \
    'call i32 @chooseU32(i32 8)' \
    'call i64 @chooseI64(i64 9)' \
    'call i64 @chooseU64(i64 10)' \
    'call i1 @chooseBool(i1 1)' \
    'call i64 @chooseMode(i64 11)' \
    'call double @chooseFloat(double 1.25)' \
    'call i64 @chooseExit(i64 23)'
do
    rg --no-ignore -Fq "$expected" "$release_ir" || {
        echo "FAIL: release IR omitted exact default call shape: $expected" >&2
        exit 1
    }
done

for rejected in unsupported_expression mutable_constant \
    unsupported_char_constant unsupported_constant_initializer; do
    set +e
    rejected_output="$(run_compiler check \
        "$FIXTURES/$rejected.seen" 2>&1)"
    rejected_status=$?
    set -e
    if [ "$rejected_status" -eq 0 ] ||
        ! grep -Fq 'E_DEFAULT_NOT_CONSTANT' <<<"$rejected_output"; then
        echo "FAIL: $rejected default did not fail semantic checking" >&2
        printf '%s\n' "$rejected_output" >&2
        exit 1
    fi
done

echo "PASS: module constants lower to typed default arguments"
