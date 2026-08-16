#!/usr/bin/env bash
# Verify the project-local, scope-bound serializer record before LD_PRELOAD.

set -euo pipefail

if [ "$#" -ne 4 ]; then
    echo "Usage: verify_fork_serializer.sh <serializer.so> <record> <artifact-root> <scope-unit>" >&2
    exit 2
fi

serializer=$1
record=$2
artifact_root=$3
scope_unit=$4
SCRIPT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}" 2>/dev/null && pwd -P)" || {
    echo "fork serializer verification: could not resolve the helper directory" >&2
    exit 126
}
serializer_source="$SCRIPT_DIR/fork_serializer.c"
selftest_source="$SCRIPT_DIR/fork_serializer_selftest.c"

fail() {
    echo "fork serializer verification: $*" >&2
    exit 126
}

[ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] ||
    fail "verified kernel-scope marker is required"
case "$scope_unit" in
    seen-memory-guard-*.scope) ;;
    *) fail "invalid or missing scope unit" ;;
esac
[ -d "$artifact_root" ] && [ ! -L "$artifact_root" ] ||
    fail "artifact root is unsafe"
canonical_root="$(cd -P -- "$artifact_root" 2>/dev/null && pwd -P)" ||
    fail "artifact root is not resolvable"
[ "$canonical_root" = "$artifact_root" ] || fail "artifact root is not canonical"

for candidate in "$serializer" "$record"; do
    case "$candidate" in
        "$canonical_root"/*) ;;
        *) fail "serializer evidence escaped the artifact root" ;;
    esac
    [ -f "$candidate" ] && [ ! -L "$candidate" ] ||
        fail "serializer evidence is missing or a symlink: $candidate"
    candidate_parent=${candidate%/*}
    candidate_base=${candidate##*/}
    canonical_candidate_parent="$(cd -P -- "$candidate_parent" 2>/dev/null && pwd -P)" ||
        fail "serializer evidence parent is not resolvable: $candidate"
    canonical_candidate="$canonical_candidate_parent/$candidate_base"
    [ "$canonical_candidate" = "$candidate" ] ||
        fail "serializer evidence traverses a symlink: $candidate"
done

command -v sha256sum >/dev/null 2>&1 || fail "sha256sum is required"

verification_root="$canonical_root/seen_serializer_verify.${BASHPID}.${RANDOM}"
case "$verification_root" in
    "$canonical_root"/seen_serializer_verify.*) ;;
    *) fail "unsafe serializer verification scratch path" ;;
esac
[ ! -e "$verification_root" ] && [ ! -L "$verification_root" ] ||
    fail "serializer verification scratch path already exists"
mkdir -m 700 -- "$verification_root" ||
    fail "could not create project-local serializer verification scratch"

read_verification_path_identity() {
    local verification_path=$1
    local destination=$2
    local identity_lines=()
    local identity=""

    mapfile -t identity_lines < <(stat -Lc '%d:%i' -- "$verification_path" 2>/dev/null)
    [ "${#identity_lines[@]}" -eq 1 ] || return 1
    identity=${identity_lines[0]}
    case "$identity" in
        *[!0-9:]*|:*|*:) return 1 ;;
    esac
    printf -v "$destination" '%s' "$identity"
}

verification_root_identity=""
read_verification_path_identity "$verification_root" \
    verification_root_identity || {

    rmdir -- "$verification_root" 2>/dev/null || true
    fail "could not identify serializer verification scratch"
}
line_count_file="$verification_root/line-count"
hash_output_file="$verification_root/hash-output"
body_file="$verification_root/record-body"
line_count_identity=""
hash_output_identity=""
body_identity=""

remove_verification_root() {
    local require_identities=${1:-0}
    local cleanup_file=""
    local expected_identity=""
    local cleanup_identity=""
    local cleanup_root_identity=""

    case "$verification_root" in
        "$canonical_root"/seen_serializer_verify.*)
            if [ -d "$verification_root" ] && [ ! -L "$verification_root" ] &&
                [ "${verification_root%/*}" = "$canonical_root" ]; then

                read_verification_path_identity "$verification_root" \
                    cleanup_root_identity || return 1
                [ "$cleanup_root_identity" = "$verification_root_identity" ] ||
                    return 1
                for cleanup_file in "$line_count_file" "$hash_output_file" "$body_file"; do
                    [ "${cleanup_file%/*}" = "$verification_root" ] || return 1
                    if [ ! -e "$cleanup_file" ] && [ ! -L "$cleanup_file" ]; then
                        [ "$require_identities" = "0" ] || return 1
                        continue
                    fi
                    [ -f "$cleanup_file" ] && [ ! -L "$cleanup_file" ] || return 1
                    case "$cleanup_file" in
                        "$line_count_file") expected_identity=${line_count_identity:-} ;;
                        "$hash_output_file") expected_identity=${hash_output_identity:-} ;;
                        "$body_file") expected_identity=${body_identity:-} ;;
                        *) return 1 ;;
                    esac
                    if [ -n "$expected_identity" ]; then
                        cleanup_identity=""
                        read_verification_path_identity "$cleanup_file" \
                            cleanup_identity || return 1
                        [ "$cleanup_identity" = "$expected_identity" ] || return 1
                    elif [ "$require_identities" = "1" ]; then
                        return 1
                    fi
                done
                rm -f -- "$line_count_file" "$hash_output_file" "$body_file" ||
                    return 1
                cleanup_root_identity=""
                read_verification_path_identity "$verification_root" \
                    cleanup_root_identity || return 1
                [ "$cleanup_root_identity" = "$verification_root_identity" ] ||
                    return 1
                rmdir -- "$verification_root" 2>/dev/null || return 1
                [ ! -e "$verification_root" ] && [ ! -L "$verification_root" ] ||
                    return 1
            else
                return 1
            fi
            ;;
        *) return 1 ;;
    esac
}

cleanup_verification_root() {
    local status=$?
    remove_verification_root 0 || status=126
    return "$status"
}
trap cleanup_verification_root EXIT
umask 077
set -o noclobber
for verification_file in "$line_count_file" "$hash_output_file" "$body_file"; do
    if ! : > "$verification_file"; then
        set +o noclobber
        fail "could not create a project-local serializer verification file"
    fi
done
set +o noclobber

validate_verification_file_identity() {
    local verification_path=$1
    local expected_identity=$2
    local current_identity=""

    [ "${verification_path%/*}" = "$verification_root" ] &&
        [ -f "$verification_path" ] && [ ! -L "$verification_path" ] ||
        return 1
    read_verification_path_identity "$verification_path" current_identity ||
        return 1
    [ "$current_identity" = "$expected_identity" ]
}

read_verification_path_identity "$line_count_file" line_count_identity ||
    fail "could not identify the line-count probe"
read_verification_path_identity "$hash_output_file" hash_output_identity ||
    fail "could not identify the hash-output probe"
read_verification_path_identity "$body_file" body_identity ||
    fail "could not identify the record-body probe"

# Keep every evidence probe serial. In particular, avoid the former
# sed/sha256sum/awk and wc/tr pipelines: their command-substitution shells
# could consume four task slots at once near the aggregate TasksMax boundary.
wc -l < "$record" > "$line_count_file" 2>/dev/null ||
    fail "could not count attestation record lines"
validate_verification_file_identity "$line_count_file" "$line_count_identity" ||
    fail "line-count probe identity changed"
record_line_count=""
IFS= read -r record_line_count _ < "$line_count_file" || true
[ "$record_line_count" = "10" ] || fail "record shape is invalid"
mapfile -t record_lines < "$record" || fail "could not read attestation record"
[ "${#record_lines[@]}" -eq 10 ] || fail "record shape is invalid"

version=""
record_serializer_sha=""
record_source_sha=""
record_selftest_source_sha=""
record_dynamic_selftest=""
record_scope_unit=""
record_memory_max=""
record_swap_max=""
record_pids_max=""
record_body_sha=""
version_count=0
serializer_sha_count=0
source_sha_count=0
selftest_source_sha_count=0
dynamic_selftest_count=0
scope_unit_count=0
memory_max_count=0
swap_max_count=0
pids_max_count=0
body_sha_count=0
for record_line in "${record_lines[@]}"; do
    case "$record_line" in
        *=*) ;;
        *) fail "record field is malformed" ;;
    esac
    record_key=${record_line%%=*}
    record_value=${record_line#*=}
    case "$record_key" in
        version)
            version_count=$((version_count + 1))
            version=$record_value
            ;;
        serializer_sha256)
            serializer_sha_count=$((serializer_sha_count + 1))
            record_serializer_sha=$record_value
            ;;
        source_sha256)
            source_sha_count=$((source_sha_count + 1))
            record_source_sha=$record_value
            ;;
        selftest_source_sha256)
            selftest_source_sha_count=$((selftest_source_sha_count + 1))
            record_selftest_source_sha=$record_value
            ;;
        dynamic_selftest)
            dynamic_selftest_count=$((dynamic_selftest_count + 1))
            record_dynamic_selftest=$record_value
            ;;
        scope_unit)
            scope_unit_count=$((scope_unit_count + 1))
            record_scope_unit=$record_value
            ;;
        verified_memory_max_bytes)
            memory_max_count=$((memory_max_count + 1))
            record_memory_max=$record_value
            ;;
        verified_memory_swap_max_bytes)
            swap_max_count=$((swap_max_count + 1))
            record_swap_max=$record_value
            ;;
        verified_pids_max)
            pids_max_count=$((pids_max_count + 1))
            record_pids_max=$record_value
            ;;
        body_sha256)
            body_sha_count=$((body_sha_count + 1))
            record_body_sha=$record_value
            ;;
        *) fail "record contains an unknown field: $record_key" ;;
    esac
done
for field_count_and_name in \
    "$version_count:version" \
    "$serializer_sha_count:serializer_sha256" \
    "$source_sha_count:source_sha256" \
    "$selftest_source_sha_count:selftest_source_sha256" \
    "$dynamic_selftest_count:dynamic_selftest" \
    "$scope_unit_count:scope_unit" \
    "$memory_max_count:verified_memory_max_bytes" \
    "$swap_max_count:verified_memory_swap_max_bytes" \
    "$pids_max_count:verified_pids_max" \
    "$body_sha_count:body_sha256"; do

    field_count=${field_count_and_name%%:*}
    field_name=${field_count_and_name#*:}
    [ "$field_count" = "1" ] ||
        fail "record field is missing or duplicated: $field_name"
done

[ "$version" = "seen-fork-serializer-attestation-v2" ] || fail "record version mismatch"
[[ "$record_serializer_sha" =~ ^[0-9a-f]{64}$ ]] || fail "invalid serializer digest"
[[ "$record_source_sha" =~ ^[0-9a-f]{64}$ ]] || fail "invalid source digest"
[[ "$record_selftest_source_sha" =~ ^[0-9a-f]{64}$ ]] || fail "invalid self-test source digest"
[[ "$record_body_sha" =~ ^[0-9a-f]{64}$ ]] || fail "invalid record digest"
[ "$record_dynamic_selftest" = "passed" ] || fail "dynamic serializer self-test evidence is missing"
[ "$record_scope_unit" = "$scope_unit" ] || fail "scope-unit binding mismatch"
case "$record_memory_max:$record_pids_max" in
    *[!0-9:]*|:*|*:) fail "invalid cgroup limits in record" ;;
esac
[ "$record_memory_max" -gt 0 ] && [ "$record_memory_max" -le 4294967296 ] ||
    fail "recorded memory.max exceeds 4 GiB"
[ "$record_swap_max" = "0" ] || fail "recorded memory.swap.max is not zero"
[ "$record_pids_max" -gt 0 ] && [ "$record_pids_max" -le 16 ] ||
    fail "recorded pids.max exceeds 16"

printf '%s\n' "${record_lines[@]:0:9}" > "$body_file" ||
    fail "could not create the project-local record-body probe"
validate_verification_file_identity "$body_file" "$body_identity" ||
    fail "record-body probe identity changed"

hash_file_into() {
    local destination=$1
    local source_file=$2
    local digest=""

    sha256sum -- "$source_file" > "$hash_output_file" 2>/dev/null ||
        fail "could not hash serializer evidence: $source_file"
    validate_verification_file_identity "$hash_output_file" \
        "$hash_output_identity" ||
        fail "hash-output probe identity changed"
    IFS=' ' read -r digest _ < "$hash_output_file" || true
    [[ "$digest" =~ ^[0-9a-f]{64}$ ]] ||
        fail "serializer evidence hash output is invalid: $source_file"
    printf -v "$destination" '%s' "$digest"
}

calculated_body_sha=""
calculated_serializer_sha=""
calculated_source_sha=""
calculated_selftest_source_sha=""
hash_file_into calculated_body_sha "$body_file"
[ "$calculated_body_sha" = "$record_body_sha" ] || fail "record checksum mismatch"
hash_file_into calculated_serializer_sha "$serializer"
[ "$calculated_serializer_sha" = "$record_serializer_sha" ] ||
    fail "serializer digest mismatch"
hash_file_into calculated_source_sha "$serializer_source"
[ "$calculated_source_sha" = "$record_source_sha" ] ||
    fail "serializer source digest mismatch"
hash_file_into calculated_selftest_source_sha "$selftest_source"
[ "$calculated_selftest_source_sha" = "$record_selftest_source_sha" ] ||
    fail "serializer self-test source digest mismatch"

current_cgroup=""
if [ -r /proc/self/cgroup ]; then
    while IFS=: read -r cgroup_hierarchy _ cgroup_path; do
        if [ "$cgroup_hierarchy" = "0" ]; then
            current_cgroup=$cgroup_path
            break
        fi
    done < /proc/self/cgroup
fi
case "$current_cgroup" in
    *"/$scope_unit"|*"/$scope_unit/"*) ;;
    *) fail "current process is not inside the recorded scope" ;;
esac

if ! remove_verification_root 1; then
    echo "fork serializer verification: could not safely remove project-local probes" >&2
    exit 126
fi
trap - EXIT
printf 'verified\n'
