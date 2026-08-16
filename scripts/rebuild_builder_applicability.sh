#!/usr/bin/env bash
# Static fail-closed applicability check for the rebuild fork serializer.

set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "Usage: rebuild_builder_applicability.sh <compiler> <serializer.so>" >&2
    exit 2
fi

compiler=$1
serializer=$2
SCRIPT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}" 2>/dev/null && pwd -P)" || {
    echo "builder applicability: could not resolve the helper directory" >&2
    exit 3
}
REPO_ROOT="$(cd -P -- "$SCRIPT_DIR/.." 2>/dev/null && pwd -P)" || {
    echo "builder applicability: could not resolve the repository root" >&2
    exit 3
}

fail() {
    echo "builder applicability: $*" >&2
    exit 3
}

[ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] ||
    fail "verified kernel-scope marker is required"
[ -x "$compiler" ] && [ -f "$compiler" ] && [ ! -L "$compiler" ] ||
    fail "compiler must be a regular non-symlink executable"
[ -f "$serializer" ] && [ ! -L "$serializer" ] ||
    fail "serializer must be a regular non-symlink file"
command -v readelf >/dev/null 2>&1 || fail "readelf is required"

probe_root=${TMPDIR:-}
case "$probe_root" in
    "$REPO_ROOT"/*) ;;
    *) fail "TMPDIR must be an explicit project-local directory" ;;
esac
[ -d "$probe_root" ] && [ ! -L "$probe_root" ] && [ -w "$probe_root" ] ||
    fail "TMPDIR is not a safe writable directory"
canonical_probe_root="$(cd -P -- "$probe_root" 2>/dev/null && pwd -P)" ||
    fail "TMPDIR is not resolvable"
[ "$canonical_probe_root" = "$probe_root" ] ||
    fail "TMPDIR traverses a symbolic link or is not canonical"

probe_stem="$probe_root/seen_builder_applicability.${BASHPID}.${RANDOM}"
mode_file="$probe_stem.mode"
headers_file="$probe_stem.headers"
compiler_symbols_file="$probe_stem.compiler-symbols"
serializer_symbols_file="$probe_stem.serializer-symbols"
mode_identity=""
headers_identity=""
compiler_symbols_identity=""
serializer_symbols_identity=""
for probe_file in "$mode_file" "$headers_file" \
    "$compiler_symbols_file" "$serializer_symbols_file"; do

    case "$probe_file" in
        "$probe_root"/seen_builder_applicability.*) ;;
        *) fail "unsafe project-local probe path" ;;
    esac
    [ ! -e "$probe_file" ] && [ ! -L "$probe_file" ] ||
        fail "project-local probe path already exists"
done
remove_probe_files() {
    local require_identities=${1:-0}
    local cleanup_file=""
    local expected_identity=""
    local cleanup_identity=""

    for cleanup_file in "$mode_file" "$headers_file" \
        "$compiler_symbols_file" "$serializer_symbols_file"; do

        case "$cleanup_file" in
            "$probe_root"/seen_builder_applicability.*)
                [ "${cleanup_file%/*}" = "$probe_root" ] || return 1
                ;;
            *) return 1 ;;
        esac
        if [ ! -e "$cleanup_file" ] && [ ! -L "$cleanup_file" ]; then
            [ "$require_identities" = "0" ] || return 1
            continue
        fi
        [ -f "$cleanup_file" ] && [ ! -L "$cleanup_file" ] || return 1
        case "$cleanup_file" in
            "$mode_file") expected_identity=${mode_identity:-} ;;
            "$headers_file") expected_identity=${headers_identity:-} ;;
            "$compiler_symbols_file") expected_identity=${compiler_symbols_identity:-} ;;
            "$serializer_symbols_file") expected_identity=${serializer_symbols_identity:-} ;;
            *) return 1 ;;
        esac
        if [ -n "$expected_identity" ]; then
            declare -F read_probe_identity >/dev/null 2>&1 || return 1
            cleanup_identity=""
            read_probe_identity "$cleanup_file" cleanup_identity || return 1
            [ "$cleanup_identity" = "$expected_identity" ] || return 1
        elif [ "$require_identities" = "1" ]; then
            return 1
        fi
    done
    rm -f -- "$mode_file" "$headers_file" "$compiler_symbols_file" \
        "$serializer_symbols_file" || return 1
    for cleanup_file in "$mode_file" "$headers_file" \
        "$compiler_symbols_file" "$serializer_symbols_file"; do

        [ ! -e "$cleanup_file" ] && [ ! -L "$cleanup_file" ] || return 1
    done
}

cleanup_probe_files() {
    local status=$?
    remove_probe_files 0 || status=3
    return "$status"
}
trap cleanup_probe_files EXIT
umask 077
set -o noclobber
for probe_file in "$mode_file" "$headers_file" \
    "$compiler_symbols_file" "$serializer_symbols_file"; do

    if ! : > "$probe_file"; then
        set +o noclobber
        fail "could not create a project-local probe file"
    fi
done
set +o noclobber

read_probe_identity() {
    local probe_path=$1
    local destination=$2
    local identity_lines=()
    local identity=""

    mapfile -t identity_lines < <(stat -Lc '%d:%i' -- "$probe_path" 2>/dev/null)
    [ "${#identity_lines[@]}" -eq 1 ] || return 1
    identity=${identity_lines[0]}
    case "$identity" in
        *[!0-9:]*|:*|*:) return 1 ;;
    esac
    printf -v "$destination" '%s' "$identity"
}

validate_probe_identity() {
    local probe_path=$1
    local expected_identity=$2
    local current_identity=""

    case "$probe_path" in
        "$probe_root"/seen_builder_applicability.*) ;;
        *) return 1 ;;
    esac
    [ "${probe_path%/*}" = "$probe_root" ] && [ -f "$probe_path" ] &&
        [ ! -L "$probe_path" ] || return 1
    read_probe_identity "$probe_path" current_identity || return 1
    [ "$current_identity" = "$expected_identity" ]
}

read_probe_identity "$mode_file" mode_identity ||
    fail "could not identify the compiler-mode probe"
read_probe_identity "$headers_file" headers_identity ||
    fail "could not identify the program-header probe"
read_probe_identity "$compiler_symbols_file" compiler_symbols_identity ||
    fail "could not identify the compiler-symbol probe"
read_probe_identity "$serializer_symbols_file" serializer_symbols_identity ||
    fail "could not identify the serializer-symbol probe"

# Write each tool result before parsing it. The previous three-stage symbol
# command substitutions required four simultaneous task slots at their peak.
# These probes use one external child at a time and Bash builtins for parsing.
stat -c '%a' "$compiler" > "$mode_file" 2>/dev/null ||
    fail "could not read compiler mode"
validate_probe_identity "$mode_file" "$mode_identity" ||
    fail "compiler-mode probe identity changed"
compiler_mode=""
IFS= read -r compiler_mode < "$mode_file" || true
case "$compiler_mode" in
    ''|*[!0-7]*) fail "could not read compiler mode" ;;
esac
if [ $((8#$compiler_mode & 8#6000)) -ne 0 ]; then
    fail "setuid/setgid compilers cannot use LD_PRELOAD containment"
fi

readelf -lW "$compiler" > "$headers_file" 2>/dev/null ||
    fail "could not inspect compiler program headers"
validate_probe_identity "$headers_file" "$headers_identity" ||
    fail "program-header probe identity changed"
has_interp=0
while IFS= read -r header_line || [ -n "$header_line" ]; do
    case "$header_line" in
        *' INTERP '*) has_interp=1; break ;;
    esac
done < "$headers_file"
if [ "$has_interp" != "1" ]; then
    fail "compiler is static or has no dynamic interpreter"
fi

readelf -Ws "$compiler" > "$compiler_symbols_file" 2>/dev/null ||
    fail "could not inspect compiler imports"
validate_probe_identity "$compiler_symbols_file" "$compiler_symbols_identity" ||
    fail "compiler-symbol probe identity changed"
readelf -Ws "$serializer" > "$serializer_symbols_file" 2>/dev/null ||
    fail "could not inspect serializer exports"
validate_probe_identity "$serializer_symbols_file" "$serializer_symbols_identity" ||
    fail "serializer-symbol probe identity changed"

declare -A imported_symbols=()
declare -A exported_symbols=()
while read -r _ _ _ _ _ _ symbol_index symbol_name _; do
    [ "$symbol_index" = "UND" ] || continue
    symbol_name=${symbol_name%%@*}
    case "$symbol_name" in
        vfork|__vfork|_Fork|clone|__clone|clone3|__clone3|\
        pthread_create|__pthread_create|thrd_create|daemon|wait|waitid|wait3|wait4|\
        fork|posix_spawn|posix_spawnp|popen|pclose|waitpid)
            imported_symbols["$symbol_name"]=1
            ;;
    esac
done < "$compiler_symbols_file"
while read -r _ _ _ _ _ _ symbol_index symbol_name _; do
    [ -n "$symbol_index" ] && [ "$symbol_index" != "UND" ] || continue
    symbol_name=${symbol_name%%@*}
    case "$symbol_name" in
        fork|posix_spawn|posix_spawnp|popen|pclose|waitpid)
            exported_symbols["$symbol_name"]=1
            ;;
    esac
done < "$serializer_symbols_file"

# These APIs can create root work outside the fork/posix_spawn/popen paths that
# the target-gated serializer controls. Descendant tools intentionally run in
# pass-through mode under the separately verified cgroup/task/toolchain caps.
# Reject rather than infer root safety from --help text.
for unsupported in vfork __vfork _Fork clone __clone clone3 __clone3 \
    pthread_create __pthread_create thrd_create daemon wait waitid wait3 wait4; do
    if [ "${imported_symbols[$unsupported]:-0}" = "1" ]; then
        fail "unsupported process/thread creation import: $unsupported"
    fi
done

for handled in fork posix_spawn posix_spawnp popen pclose waitpid; do
    if [ "${imported_symbols[$handled]:-0}" = "1" ] &&
        [ "${exported_symbols[$handled]:-0}" != "1" ]; then

        fail "serializer does not interpose imported symbol: $handled"
    fi
done

if ! remove_probe_files 1; then
    echo "builder applicability: could not safely remove project-local probes" >&2
    exit 3
fi
trap - EXIT
printf 'applicable\n'
