#!/usr/bin/env bash
# Build current-source ARM ELF inputs; this is not native qualification.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
SELF="$ROOT_DIR/scripts/build_linux_arm64_verification_inputs.sh"
CHECKER="$ROOT_DIR/scripts/ci_linux_arm64_inputs.py"
fail() { printf 'linux-arm64-inputs: %s\n' "$*" >&2; exit 126; }

if [ "${SEEN_ARM_INPUTS_INNER:-0}" != 1 ]; then
    CANDIDATE="" OUTPUT=""
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --candidate) [ "$#" -ge 2 ] && [ -z "$CANDIDATE" ] || fail 'expected one --candidate DIR'; CANDIDATE=$2; shift 2 ;;
            --output) [ "$#" -ge 2 ] && [ -z "$OUTPUT" ] || fail 'expected one --output ARCHIVE'; OUTPUT=$2; shift 2 ;;
            *) fail 'usage: build_linux_arm64_verification_inputs.sh --candidate DIR --output ARCHIVE' ;;
        esac
    done
    [ -n "$CANDIDATE" ] && [ -n "$OUTPUT" ] || fail '--candidate and --output are required'
    case "$(uname -s):$(uname -m)" in Linux:x86_64|Linux:amd64) ;; *) fail 'cross-build requires Linux x64 bootstrap host' ;; esac
    # Do not silently use the compiler fallback's guessed sysroot locations.
    [[ "${SEEN_LINUX_ARM64_SYSROOT:-}" =~ ^/[a-zA-Z0-9_./-]+$ ]] &&
        [ -d "$SEEN_LINUX_ARM64_SYSROOT" ] ||
        fail 'SEEN_LINUX_ARM64_SYSROOT must name an existing absolute shell-safe sysroot'
    if [ -n "${SEEN_LINUX_ARM64_GCC_TOOLCHAIN:-}" ]; then
        [[ "$SEEN_LINUX_ARM64_GCC_TOOLCHAIN" =~ ^/[a-zA-Z0-9_./-]+$ ]] &&
            [ -d "$SEEN_LINUX_ARM64_GCC_TOOLCHAIN" ] ||
            fail 'SEEN_LINUX_ARM64_GCC_TOOLCHAIN must name an existing absolute shell-safe toolchain'
    fi
    export SEEN_ARM_INPUTS_CANDIDATE="$CANDIDATE" SEEN_ARM_INPUTS_OUTPUT="$OUTPUT"
    export SEEN_STAGE_BUILDER="$CANDIDATE/seen" SEEN_STAGE_BUILDER_ONLY=1
    export SEEN_STAGE_BUILDER_PACKAGE_CLIENT="$CANDIDATE/seen-pkg" SEEN_PACKAGE_CLIENT="$CANDIDATE/seen-pkg"
    export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR" SEEN_ARM_INPUTS_INNER=1
    python3 - "$ROOT_DIR" "$CANDIDATE" "$OUTPUT" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path(sys.argv[1]) / "scripts"))
import ci_linux_arm64_inputs as inputs
root, candidate, output = map(Path, sys.argv[1:])
inputs.candidate.ignored_output(root, output)
inputs.binding(root, candidate)
PY
    # Routing is not attestation: safe_rebuild derives live RAM limits and
    # creates the scope-bound serializer independently of the inner marker.
    exec bash "$ROOT_DIR/scripts/safe_rebuild.sh" --tier quick --regression-script "$SELF"
fi

[ "$#" -eq 0 ] || fail 'inner build does not accept arguments'
CANDIDATE="${SEEN_ARM_INPUTS_CANDIDATE:?missing candidate routing}"
OUTPUT="${SEEN_ARM_INPUTS_OUTPUT:?missing output routing}"
[ "${COMPILER:-}" = "$CANDIDATE/seen" ] || fail 'regression builder differs from the exact candidate'
if [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != linux-arm64-inputs ]; then
    exec bash "$ROOT_DIR/scripts/run_capped_regression.sh" linux-arm64-inputs \
        --compiler "$CANDIDATE/seen" -- bash "$SELF"
fi
bash "$ROOT_DIR/scripts/run_capped_regression.sh" --verify-active linux-arm64-inputs --compiler "$CANDIDATE/seen"
# The canonical verifier reads cgroup/kernel/serializer state rather than trusting markers.
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
export SEEN_PACKAGE_CLIENT="$CANDIDATE/seen-pkg"
export GOMAXPROCS=1 GOFLAGS='-p=1' SEEN_JOBS=1 SEEN_OPT_JOBS=1
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1 BLIS_NUM_THREADS=1
export CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 RPM_BUILD_NCPUS=1
export SEEN_LLD_THREADS=1 SEEN_THINLTO_JOBS=1
[ "${SEEN_MAIN_VMEM_KB:?}" -ge 524288 ] || fail 'live main cap cannot support the bounded package-client builder'
SEEN_PACKAGE_CLIENT_VMEM_KB=$SEEN_OPT_VMEM_KB
[ "$SEEN_PACKAGE_CLIENT_VMEM_KB" -le "$SEEN_MAIN_VMEM_KB" ] || SEEN_PACKAGE_CLIENT_VMEM_KB=$SEEN_MAIN_VMEM_KB
export SEEN_PACKAGE_CLIENT_VMEM_KB

work="$(mktemp -d "$SEEN_ARTIFACT_ROOT/linux-arm64-inputs.XXXXXXXX")"
mkdir "$work/build" "$work/proof"
# Each proof/log file is bounded; a full log fails the build rather than hiding errors.
ulimit -S -f 65536
cleanup() {
    local status=$?
    trap - EXIT
    printf '%s\n' "$status" > "$work/proof/exit-status"
    rm -rf -- "$work/build"
    printf 'ARM cross-build proof (not native qualification): %s\n' "$work/proof" >&2
    exit "$status"
}
trap cleanup EXIT
cd "$ROOT_DIR"
binding() {
    python3 - "$ROOT_DIR" "$CANDIDATE" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path(sys.argv[1]) / "scripts"))
import ci_linux_arm64_inputs as inputs
print(inputs.encode(inputs.binding(Path(sys.argv[1]), Path(sys.argv[2]))).decode(), end="")
PY
}
binding > "$work/proof/before.json"
record_scope() {
    python3 - "$ROOT_DIR" "$1" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path(sys.argv[1]) / "scripts"))
import ci_bootstrap_candidate as candidate
from ci_candidate_io import publish_json
publish_json(Path(sys.argv[2]), candidate.live_scope(Path(sys.argv[1])))
PY
}
record_scope "$work/proof/scope-before.json"
version="$(python3 - "$ROOT_DIR" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path(sys.argv[1]) / "scripts"))
from ci_linux_arm64_inputs import source_version
print(source_version(Path(sys.argv[1])))
PY
)"
compile=(bash "$ROOT_DIR/scripts/run_attested_seen.sh" "$CANDIDATE/seen"
    compile "$ROOT_DIR/compiler_seen/src/main_compiler.seen" "$work/build/seen"
    --target=linux-arm64 --fast --no-cache)
client=(bash "$ROOT_DIR/scripts/build_package_client.sh" --version "$version"
    --goos linux --goarch arm64 --output "$work/build/seen-pkg")
{
    printf 'sysroot=%q\ngcc_toolchain=%q\n' "$SEEN_LINUX_ARM64_SYSROOT" "${SEEN_LINUX_ARM64_GCC_TOOLCHAIN:-}"
    printf '%q ' "${compile[@]}"; printf '\n'
    printf '%q ' "${client[@]}"; printf '\n'
} > "$work/proof/commands.log"
# The attested runner selects exactly one supported serial-worker flag family.
# Preserve genuine compiler/toolchain failures; never repair emitted IR or source.
"${compile[@]}" > "$work/proof/compiler.log" 2>&1
"${client[@]}" > "$work/proof/client.log" 2>&1
record_scope "$work/proof/scope-after.json"
cmp -- "$work/proof/scope-before.json" "$work/proof/scope-after.json" || fail 'live scope changed during cross-build'
binding > "$work/proof/after.json"
cmp -- "$work/proof/before.json" "$work/proof/after.json" || fail 'source or candidate changed during cross-build'
python3 "$CHECKER" create --root "$ROOT_DIR" --candidate "$CANDIDATE" \
    --compiler "$work/build/seen" --client "$work/build/seen-pkg" --output "$OUTPUT"
printf 'Created cross-built-unverified Linux ARM64 inputs: %s\n' "$OUTPUT"
