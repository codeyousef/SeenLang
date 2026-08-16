#!/usr/bin/env bash
# Create project-local wrappers that apply the <=2 GiB helper cap before exec.

set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "Usage: prepare_bounded_toolchain.sh <verified-artifact-root>" >&2
    exit 2
fi

artifact_root=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
ARTIFACT_HELPER="$SCRIPT_DIR/artifact_root.sh"

fail() {
    echo "bounded toolchain: $*" >&2
    exit 126
}

[ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] ||
    fail "verified kernel scope is required"
[ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" = "1" ] ||
    [ "${SEEN_REBUILD_AGGREGATE_SCOPE_ACTIVE:-0}" = "1" ] ||
    fail "verified aggregate-scope marker is required"
[ -f "$ARTIFACT_HELPER" ] || fail "artifact-root helper is missing"
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER"

seen_artifact_root_init "$REPO_ROOT" || fail "artifact-root validation failed"
[ "$artifact_root" = "$SEEN_ARTIFACT_ROOT" ] ||
    fail "requested root does not match the validated artifact root"
[ -d "$artifact_root" ] && [ ! -L "$artifact_root" ] ||
    fail "artifact root is unsafe"
canonical_root=$(seen_artifact_canonical_dir "$artifact_root") ||
    fail "artifact root is not canonical"
[ "$canonical_root" = "$artifact_root" ] || fail "artifact root is not canonical"

toolchain_dir="$artifact_root/bounded-toolchain"
case "$toolchain_dir" in
    "$canonical_root"/*) ;;
    *) fail "toolchain directory escaped the artifact root" ;;
esac
[ ! -L "$toolchain_dir" ] || fail "toolchain directory is a symlink"
mkdir -p -- "$toolchain_dir"
[ -d "$toolchain_dir" ] && [ ! -L "$toolchain_dir" ] ||
    fail "toolchain directory is unsafe"

find_real_tool() {
    local name=$1
    local candidate canonical_candidate
    while IFS= read -r candidate; do
        [ -n "$candidate" ] || continue
        case "$candidate" in
            "$canonical_root"/*) continue ;;
        esac
        [ -x "$candidate" ] || continue
        canonical_candidate=$(readlink -f -- "$candidate" 2>/dev/null || true)
        [ -n "$canonical_candidate" ] && [ -x "$canonical_candidate" ] || continue
        case "$canonical_candidate" in
            "$canonical_root"/*) continue ;;
        esac
        printf '%s\n' "$canonical_candidate"
        return 0
    done < <(type -a -p -- "$name" 2>/dev/null || true)
    return 1
}

write_wrapper() {
    local name=$1
    local real_tool=$2
    local output="$toolchain_dir/$name"
    local tmp
    local quoted_real
    local quoted_name

    [ ! -L "$output" ] || fail "wrapper target is a symlink: $output"
    tmp=$(mktemp "$toolchain_dir/.${name}.XXXXXX") || fail "could not create wrapper"
    printf -v quoted_real '%q' "$real_tool"
    printf -v quoted_name '%q' "$name"
    {
        printf '%s\n' '#!/usr/bin/env bash'
        printf '%s\n' 'set -euo pipefail'
        printf '%s\n' 'limit_kb=${SEEN_OPT_VMEM_KB:-}'
        printf '%s\n' 'case "$limit_kb" in'
        printf '%s\n' "    ''|*[!0-9]*) echo 'RESOURCE STOP: missing optimizer/helper virtual-memory cap' >&2; exit 126 ;;"
        printf '%s\n' 'esac'
        printf '%s\n' "if [ \"\$limit_kb\" -le 0 ] || [ \"\$limit_kb\" -gt 2097152 ]; then echo 'RESOURCE STOP: invalid optimizer/helper virtual-memory cap' >&2; exit 126; fi"
        printf '%s\n' "if ! ulimit -S -v \"\$limit_kb\" 2>/dev/null; then echo 'RESOURCE STOP: could not apply optimizer/helper virtual-memory cap' >&2; exit 126; fi"
        printf '%s\n' 'active_kb=$(ulimit -S -v 2>/dev/null || true)'
        printf '%s\n' 'case "$active_kb" in'
        printf '%s\n' "    ''|*[!0-9]*) echo 'RESOURCE STOP: could not read back optimizer/helper virtual-memory cap' >&2; exit 126 ;;"
        printf '%s\n' 'esac'
        printf '%s\n' "if [ \"\$active_kb\" -gt \"\$limit_kb\" ]; then echo 'RESOURCE STOP: optimizer/helper cap read-back exceeds request' >&2; exit 126; fi"
        printf 'tool_name=%s\n' "$quoted_name"
        printf '%s\n' 'case "$tool_name" in'
        printf '%s\n' '    ld.lld|wasm-ld)'
        printf '%s\n' "        [ \"\${SEEN_LLD_THREADS:-}\" = 1 ] || { echo 'RESOURCE STOP: LLD thread cap is missing' >&2; exit 126; }"
        printf '%s\n' "        [ \"\${SEEN_THINLTO_JOBS:-}\" = 1 ] || { echo 'RESOURCE STOP: ThinLTO job cap is missing' >&2; exit 126; }"
        printf '        exec %s --threads=1 --thinlto-jobs=1 "$@"\n' "$quoted_real"
        printf '%s\n' '        ;;'
        printf '%s\n' '    clang|clang++|cc|c++)'
        printf '%s\n' '        link_step=1'
        printf '%s\n' '        thinlto_step=0'
        printf '%s\n' '        lld_step=0'
        printf '%s\n' '        for argument in "$@"; do'
        printf '%s\n' '            case "$argument" in'
        printf '%s\n' '                -c|-S|-E|-M|-MM) link_step=0 ;;'
        printf '%s\n' '                -flto=thin) thinlto_step=1 ;;'
        printf '%s\n' '                -fuse-ld=lld) lld_step=1 ;;'
        printf '%s\n' '            esac'
        printf '%s\n' '        done'
        printf '%s\n' '        if [ "$link_step" = 1 ] && [ "$thinlto_step" = 1 ]; then'
        printf '%s\n' "            [ \"\${SEEN_THINLTO_JOBS:-}\" = 1 ] || { echo 'RESOURCE STOP: ThinLTO job cap is missing' >&2; exit 126; }"
        printf '%s\n' '            set -- "$@" -flto-jobs=1'
        printf '%s\n' '        fi'
        printf '%s\n' '        if [ "$link_step" = 1 ] && [ "$lld_step" = 1 ]; then'
        printf '%s\n' "            [ \"\${SEEN_LLD_THREADS:-}\" = 1 ] || { echo 'RESOURCE STOP: LLD thread cap is missing' >&2; exit 126; }"
        printf '%s\n' "            [ \"\${SEEN_THINLTO_JOBS:-}\" = 1 ] || { echo 'RESOURCE STOP: ThinLTO job cap is missing' >&2; exit 126; }"
        printf '%s\n' '            set -- "$@" -Wl,--threads=1 -Wl,--thinlto-jobs=1'
        printf '%s\n' '        fi'
        printf '        exec %s "$@"\n' "$quoted_real"
        printf '%s\n' '        ;;'
        printf '%s\n' '    *)'
        printf '        exec %s "$@"\n' "$quoted_real"
        printf '%s\n' '        ;;'
        printf '%s\n' 'esac'
    } > "$tmp"
    chmod 755 "$tmp"
    mv -f -- "$tmp" "$output"
}

wrapped=0
for tool_name in opt llvm-as llvm-link llc clang clang++ cc c++ gcc g++ \
    ld ld.lld lld wasm-ld glslc; do
    real_tool=$(find_real_tool "$tool_name" || true)
    [ -n "$real_tool" ] || continue
    write_wrapper "$tool_name" "$real_tool"
    wrapped=$((wrapped + 1))
done
[ "$wrapped" -gt 0 ] || fail "no optimizer/link-driving tools were discovered"

printf '%s\n' "$toolchain_dir"
