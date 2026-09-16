#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-integer-string-width
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
SOURCE="$ROOT_DIR/tests/fixtures/integer-string-width/lifecycle.seen"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/integer-string-width.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/integer-string-width.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved integer-string-width artifacts: $WORK_DIR" >&2
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

run_compiler check "$SOURCE" >"$WORK_DIR/frontend.log" 2>&1
for profile in fast release; do
    output="$WORK_DIR/lifecycle-$profile"
    ir_dir="$WORK_DIR/ir-$profile"
    log="$WORK_DIR/$profile.log"
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1
        --opt-jobs 1 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler compile "$SOURCE" "$output" "${flags[@]}" >"$log" 2>&1
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1
    grep -Fq 'PASS: fixed-width integer string conversion' "$log"

    ir="$ir_dir/seen_module_0.ll"
    [ -s "$ir" ] || { echo "FAIL: missing integer conversion IR" >&2; exit 1; }
    for width in 8 16 32; do
        grep -Eq " = sext i$width .* to i64" "$ir"
        grep -Eq " = zext i$width .* to i64" "$ir"
    done
    grep -Eq 'call %SeenString @seen_int_to_string\(i64 %' "$ir"
    grep -Eq 'call %SeenString @seen_uint_to_string\(i64 %' "$ir"
done

echo 'PASS: fixed-width integer string conversion contract'
