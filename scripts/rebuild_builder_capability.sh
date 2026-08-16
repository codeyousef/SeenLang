#!/usr/bin/env bash
# Classify only the worker-control schema advertised by a Seen compiler.
#
# This probe does not prove that a binary honors the flags it prints. Callers
# must independently enforce a verified aggregate kernel scope and a
# hash-bound/project-local fork serializer (or a future trusted attestation)
# before allowing the binary to compile anything.

set -u

if [ "$#" -ne 1 ]; then
    echo "Usage: rebuild_builder_capability.sh <compiler>" >&2
    exit 2
fi

builder=$1
if [ ! -x "$builder" ]; then
    echo "builder capability: not executable: $builder" >&2
    exit 2
fi
if ! command -v timeout >/dev/null 2>&1; then
    echo "builder capability: timeout is required for bounded probing" >&2
    exit 2
fi

SCRIPT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}" 2>/dev/null && pwd -P)" || {
    echo "builder capability: could not resolve the helper directory" >&2
    exit 2
}
REPO_ROOT="$(cd -P -- "$SCRIPT_DIR/.." 2>/dev/null && pwd -P)" || {
    echo "builder capability: could not resolve the repository root" >&2
    exit 2
}
probe_root=${TMPDIR:-}
case "$probe_root" in
    "$REPO_ROOT"/*) ;;
    *)
        echo "builder capability: TMPDIR must be an explicit project-local directory" >&2
        exit 2
        ;;
esac
if [ ! -d "$probe_root" ] || [ -L "$probe_root" ] || [ ! -w "$probe_root" ]; then
    echo "builder capability: TMPDIR is not a safe writable directory" >&2
    exit 2
fi
canonical_probe_root="$(cd -P -- "$probe_root" 2>/dev/null && pwd -P)" || {
    echo "builder capability: TMPDIR is not resolvable" >&2
    exit 2
}
if [ "$canonical_probe_root" != "$probe_root" ]; then
    echo "builder capability: TMPDIR traverses a symbolic link or is not canonical" >&2
    exit 2
fi

help_file="$probe_root/seen_builder_capability.${BASHPID}.${RANDOM}.help"
case "$help_file" in
    "$probe_root"/seen_builder_capability.*.help) ;;
    *)
        echo "builder capability: unsafe help probe path" >&2
        exit 2
        ;;
esac
umask 077
set -o noclobber
if ! : > "$help_file"; then
    set +o noclobber
    echo "builder capability: could not create the project-local help probe" >&2
    exit 2
fi
set +o noclobber

read_help_file_identity() {
    local destination=$1
    local identity_lines=()
    local identity=""

    mapfile -t identity_lines < <(stat -Lc '%d:%i' -- "$help_file" 2>/dev/null)
    [ "${#identity_lines[@]}" -eq 1 ] || return 1
    identity=${identity_lines[0]}
    case "$identity" in
        *[!0-9:]*|:*|*:) return 1 ;;
    esac
    printf -v "$destination" '%s' "$identity"
}

initial_help_identity=""
read_help_file_identity initial_help_identity || {
    if [ -f "$help_file" ] && [ ! -L "$help_file" ] &&
        [ "${help_file%/*}" = "$probe_root" ]; then

        rm -f -- "$help_file" 2>/dev/null || true
    fi
    echo "builder capability: could not identify the project-local help probe" >&2
    exit 2
}

remove_help_file() {
    local cleanup_identity=""

    case "$help_file" in
        "$probe_root"/seen_builder_capability.*.help) ;;
        *) return 1 ;;
    esac
    [ "${help_file%/*}" = "$probe_root" ] && [ -f "$help_file" ] &&
        [ ! -L "$help_file" ] || return 1
    read_help_file_identity cleanup_identity || return 1
    [ "$cleanup_identity" = "$initial_help_identity" ] || return 1
    rm -f -- "$help_file" || return 1
    [ ! -e "$help_file" ] && [ ! -L "$help_file" ]
}

cleanup_help_file() {
    local status=$?
    remove_help_file || status=2
    return "$status"
}
trap cleanup_help_file EXIT

finish_classification() {
    local classification=$1

    # Callers consume stdout as an authorization token. Do not emit it until
    # integrity cleanup succeeds, even if a caller accidentally masks status.
    if ! remove_help_file; then
        echo "builder capability: could not safely remove the help probe" >&2
        exit 2
    fi
    trap - EXIT
    printf '%s\n' "$classification"
    exit 0
}

# Keep the timeout and compiler as the only two simultaneous children. A
# command substitution here adds another shell task, which is unsafe near the
# aggregate TasksMax=16 boundary used by bootstrap acceptance.
probe_succeeded=1
if ! timeout -k 1 10 "$builder" --help > "$help_file" 2>/dev/null; then
    probe_succeeded=0
fi
current_help_identity=""
if [ ! -f "$help_file" ] || [ -L "$help_file" ] ||
    [ "${help_file%/*}" != "$probe_root" ] ||
    ! read_help_file_identity current_help_identity ||
    [ "$current_help_identity" != "$initial_help_identity" ]; then

    echo "builder capability: help probe identity changed during classification" >&2
    exit 2
fi
if [ "$probe_succeeded" != "1" ]; then
    finish_classification serializer-required
fi
help_text=""
while IFS= read -r help_line || [ -n "$help_line" ]; do
    help_text+="${help_text:+$'\n'}$help_line"
done < "$help_file"
if [[ "$help_text" == *--jobs* && "$help_text" == *--opt-jobs* ]]; then
    finish_classification advertised-jobs
fi
if [[ "$help_text" == *--no-fork* ]]; then
    finish_classification advertised-no-fork
fi

finish_classification serializer-required
