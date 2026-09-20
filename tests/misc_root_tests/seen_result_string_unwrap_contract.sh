#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-result-string-unwrap
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
FIXTURES="$ROOT_DIR/tests/fixtures/result-string-unwrap"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/result-string-unwrap.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/result-string-unwrap.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved Result String artifacts: $WORK_DIR" >&2
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
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1)
    if [ "$profile" = fast ]; then
        flags+=(--fast)
    else
        flags+=(--release --lto=thin)
    fi
    for fixture in same_module cross_module; do
        source="$FIXTURES/$fixture.seen"
        binary="$WORK_DIR/$fixture-$profile"
        ir_dir="$WORK_DIR/$fixture-$profile-ir"
        run_compiler check "$source"
        run_compiler compile "$source" "$binary" "${flags[@]}" \
            --emit-module-ir-dir "$ir_dir"
        timeout --foreground --kill-after=5s 120s "$binary"
        if rg --no-ignore -q '@ptr_length' "$ir_dir"; then
            echo "FAIL: $fixture $profile emitted ptr_length" >&2
            exit 1
        fi
        rg --no-ignore -q 'call i64 @Result_unwrap\(ptr ' "$ir_dir" || {
            echo "FAIL: $fixture $profile omitted Result_unwrap" >&2
            exit 1
        }
        rg --no-ignore -q 'load %SeenString, ptr ' "$ir_dir" || {
            echo "FAIL: $fixture $profile lost the String payload layout" >&2
            exit 1
        }
        rg --no-ignore -q 'call i64 @seen_length\(%SeenString ' "$ir_dir" || {
            echo "FAIL: $fixture $profile omitted typed String.length" >&2
            exit 1
        }
    done

    source="$FIXTURES/packaged_readtext.seen"
    binary="$WORK_DIR/packaged-readtext-$profile"
    ir_dir="$WORK_DIR/packaged-readtext-$profile-ir"
    printf '{}' > "$WORK_DIR/input.json"
    run_compiler check "$source"
    run_compiler compile "$source" "$binary" "${flags[@]}" \
        --emit-module-ir-dir "$ir_dir"
    (cd "$WORK_DIR" && timeout --foreground --kill-after=5s 120s "$binary")
    if rg --no-ignore -q '@ptr_length' "$ir_dir"; then
        echo "FAIL: packaged readText $profile emitted ptr_length" >&2
        exit 1
    fi
    rg --no-ignore -q 'call i64 @seen_length\(%SeenString ' "$ir_dir" || {
        echo "FAIL: packaged readText $profile omitted typed String.length" >&2
        exit 1
    }
done

echo 'PASS: Result and packaged readText unwrap preserve String lowering'
