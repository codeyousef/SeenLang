#!/usr/bin/env bash
# Cross-build current-source Windows IOCP fixtures; this is not native qualification.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
SELF="$ROOT_DIR/scripts/build_windows_iocp_inputs.sh"
CHECKER="$ROOT_DIR/scripts/ci_windows_iocp.py"
fail() { printf 'windows-iocp-inputs: %s\n' "$*" >&2; exit 126; }

if [ "${SEEN_WINDOWS_INPUTS_INNER:-0}" != 1 ]; then
    CANDIDATE="" OUTPUT=""
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --candidate) [ "$#" -ge 2 ] && [ -z "$CANDIDATE" ] || fail 'expected one --candidate DIR'; CANDIDATE=$2; shift 2 ;;
            --output) [ "$#" -ge 2 ] && [ -z "$OUTPUT" ] || fail 'expected one --output ARCHIVE'; OUTPUT=$2; shift 2 ;;
            *) fail 'usage: build_windows_iocp_inputs.sh --candidate DIR --output ARCHIVE' ;;
        esac
    done
    [ -n "$CANDIDATE" ] && [ -n "$OUTPUT" ] || fail '--candidate and --output are required'
    case "$(uname -s):$(uname -m)" in Linux:x86_64|Linux:amd64) ;; *) fail 'cross-build requires Linux x64 bootstrap host' ;; esac
    command -v x86_64-w64-mingw32-gcc >/dev/null || fail 'the MinGW-w64 x86-64 toolchain is required'
    python3 "$CHECKER" binding --root "$ROOT_DIR" --candidate "$CANDIDATE" --output "$OUTPUT" >/dev/null
    export SEEN_WINDOWS_INPUTS_CANDIDATE="$CANDIDATE" SEEN_WINDOWS_INPUTS_OUTPUT="$OUTPUT"
    export SEEN_STAGE_BUILDER="$CANDIDATE/seen" SEEN_STAGE_BUILDER_ONLY=1
    export SEEN_STAGE_BUILDER_PACKAGE_CLIENT="$CANDIDATE/seen-pkg" SEEN_PACKAGE_CLIENT="$CANDIDATE/seen-pkg"
    export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR" SEEN_WINDOWS_INPUTS_INNER=1
    # Routing is not attestation: safe_rebuild derives live RAM limits and
    # creates the scope-bound serializer independently of the inner marker.
    exec bash "$ROOT_DIR/scripts/safe_rebuild.sh" --regression-script "$SELF"
fi

[ "$#" -eq 0 ] || fail 'inner build does not accept arguments'
CANDIDATE="${SEEN_WINDOWS_INPUTS_CANDIDATE:?missing candidate routing}"
OUTPUT="${SEEN_WINDOWS_INPUTS_OUTPUT:?missing output routing}"
[ "${COMPILER:-}" = "$CANDIDATE/seen" ] || fail 'regression builder differs from the exact candidate'
if [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != windows-iocp-inputs ]; then
    exec bash "$ROOT_DIR/scripts/run_capped_regression.sh" windows-iocp-inputs \
        --compiler "$CANDIDATE/seen" -- bash "$SELF"
fi
bash "$ROOT_DIR/scripts/run_capped_regression.sh" --verify-active windows-iocp-inputs --compiler "$CANDIDATE/seen"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
export SEEN_JOBS=1 SEEN_OPT_JOBS=1 SEEN_LLD_THREADS=1 SEEN_THINLTO_JOBS=1

work="$(mktemp -d "$SEEN_ARTIFACT_ROOT/windows-iocp-inputs.XXXXXXXX")"
mkdir "$work/build" "$work/proof"
ulimit -S -f 65536
cleanup() {
    local status=$?
    trap - EXIT
    printf '%s\n' "$status" > "$work/proof/exit-status"
    rm -rf -- "$work/build"
    printf 'Windows cross-build proof (not native qualification): %s\n' "$work/proof" >&2
    exit "$status"
}
trap cleanup EXIT
cd "$ROOT_DIR"
python3 "$CHECKER" binding --root "$ROOT_DIR" --candidate "$CANDIDATE" --output "$OUTPUT" > "$work/proof/before.json"
fixtures="$ROOT_DIR/tests/cross_platform_asynchronous_i_o"
for profile in fast release; do
    flags=()
    [ "$profile" = fast ] && flags=(--fast)
    for name in native contract; do
        source=$fixtures/iocp_windows.seen
        [ "$name" = contract ] && source=$fixtures/iocp_contract.seen
        # Preserve genuine compiler/linker failures; never repair emitted IR or source.
        bash "$ROOT_DIR/scripts/run_attested_seen.sh" "$CANDIDATE/seen" compile "$source" \
            "$work/build/iocp-$name-$profile.exe" --target windows-x86_64 --target-cpu=x86-64 \
            --no-cache --frozen "${flags[@]}" > "$work/proof/$name-$profile.log" 2>&1 ||
            fail "Windows $name $profile cross-build failed; see $work/proof/$name-$profile.log"
    done
done
python3 "$CHECKER" binding --root "$ROOT_DIR" --candidate "$CANDIDATE" --output "$OUTPUT" > "$work/proof/after.json"
cmp -- "$work/proof/before.json" "$work/proof/after.json" || fail 'source or candidate changed during cross-build'
python3 "$CHECKER" create --root "$ROOT_DIR" --candidate "$CANDIDATE" \
    --inputs "$work/build" --output "$OUTPUT"
printf 'Created cross-built-unverified Windows IOCP inputs: %s\n' "$OUTPUT"
