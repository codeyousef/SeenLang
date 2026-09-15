#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-transitive-repr-c-aggregate-layout
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE_DIR="$ROOT_DIR/tests/fixtures/fel-1583"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/transitive-layout.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/transitive-layout.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved transitive layout artifacts: $WORK_DIR" >&2
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

for fixture in same_module direct_module transitive_module; do
    for profile in fast release; do
        source_file="$FIXTURE_DIR/$fixture.seen"
        output="$WORK_DIR/$fixture-$profile"
        ir_dir="$WORK_DIR/ir-$fixture-$profile"
        log="$WORK_DIR/$fixture-$profile.log"
        flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1
            --emit-module-ir-dir "$ir_dir")
        if [ "$profile" = release ]; then
            flags+=(--release --lto=thin)
        else
            flags+=(--fast)
        fi
        run_compiler check "$source_file" >"$log" 2>&1
        run_compiler compile "$source_file" "$output" "${flags[@]}" \
            >>"$log" 2>&1
        timeout --foreground --kill-after=5s 60s "$output" \
            >>"$log" 2>&1
        if [ "$fixture" = transitive_module ]; then
            grep -Fxq '1271398400/5120' "$log"
        fi
    done
done

TRANSITIVE_IR="$WORK_DIR/transitive.ll"
find "$WORK_DIR/ir-transitive_module-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$TRANSITIVE_IR"
grep -Eq '%BufferView = type \{ i32, i32, i64, i64 \}' "$TRANSITIVE_IR"
grep -Eq '%ResidentTensor = type \{ %SeenString, %BufferView, %BufferView, i64, i64 \}' \
    "$TRANSITIVE_IR"
grep -Eq 'getelementptr inbounds \{ %SeenString, %BufferView, %BufferView, i64, i64 \}, ptr .* i32 0, i32 (3|4)' \
    "$TRANSITIVE_IR"
if grep -Eq 'getelementptr inbounds \{ %SeenString, i64, i64, i64, i64 \}' \
    "$TRANSITIVE_IR"; then
    echo "FAIL: transitive repr(C) aggregate fields collapsed to i64" >&2
    exit 1
fi

SAME_IR="$WORK_DIR/same.ll"
find "$WORK_DIR/ir-same_module-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$SAME_IR"
grep -Eq '%LocalOwner = type \{ %LocalView, i64 \}' "$SAME_IR"
grep -Eq '%LocalView = type \{ i32, i64 \}' "$SAME_IR"

echo "PASS: FEL-1583 transitive repr(C) aggregate layout contract"
