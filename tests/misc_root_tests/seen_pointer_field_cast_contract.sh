#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-pointer-field-cast
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE_ROOT="$ROOT_DIR/tests/fixtures/fel-1571"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/pointer-field-cast.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/pointer-field-cast.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved pointer-field cast artifacts: $WORK_DIR" >&2
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

compile_case() {
    local source_name=$1
    local profile=$2
    local stem=${source_name%.seen}
    local output="$WORK_DIR/$stem-$profile"
    local ir_dir="$WORK_DIR/ir-$stem-$profile"
    local log="$WORK_DIR/$stem-$profile.log"
    local flags=(--no-cache --frozen --target-cpu=x86-64
        --jobs 1 --opt-jobs 1 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$FIXTURE_ROOT/$source_name" >"$log" 2>&1
    run_compiler compile "$FIXTURE_ROOT/$source_name" "$output" \
        "${flags[@]}" >>"$log" 2>&1
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1
}

for profile in fast release; do
    compile_case direct_pointer_cast.seen "$profile"
    compile_case cross_module_pointer_cast.seen "$profile"
    compile_case result_pointer_cast.seen "$profile"
done

for stem in direct_pointer_cast cross_module_pointer_cast result_pointer_cast; do
    combined_ir="$WORK_DIR/$stem.ll"
    find "$WORK_DIR/ir-$stem-release" -maxdepth 1 -type f \
        -name '*.ll' -print0 | sort -z | xargs -0 cat >"$combined_ir"
    grep -Eq '^%(LocalPointerView|CrossModulePointerView) = type \{ ptr,' \
        "$combined_ir"
    grep -Fq '= ptrtoint ptr ' "$combined_ir"
    grep -Fq 'to i64 ; repr(C) pointer field' "$combined_ir"
done

echo "PASS: FEL-1571 repr(C) pointer-field integer cast contract"
