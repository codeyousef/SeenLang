#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-signed-range-termination
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"

WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/signed-range.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/signed-range.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] && rm -rf -- "$WORK_DIR"
            else
                echo "Preserved signed-range artifacts: $WORK_DIR" >&2
            fi
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }

for fixture in signed_range_termination for_continue_control_flow; do
    source="$ROOT_DIR/tests/compiler_regressions/$fixture.seen"
    run_compiler check "$source"
    for profile in fast release; do
        output="$WORK_DIR/$fixture-$profile"
        ir_dir="$WORK_DIR/$fixture-ir-$profile"
        flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1
            --emit-module-ir-dir "$ir_dir")
        if [ "$profile" = release ]; then
            flags+=(--release --lto=thin)
        else
            flags+=(--fast)
        fi
        run_compiler compile "$source" "$output" "${flags[@]}"
        timeout --foreground --kill-after=5s 30s "$output"
    done
done
echo 'PASS: signed ranges and continued indexed loops terminate in fast and ThinLTO release builds'
