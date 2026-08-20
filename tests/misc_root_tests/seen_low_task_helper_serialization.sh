#!/usr/bin/env bash
# No-build regression for helper probes used near the TasksMax=24 boundary.

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
CAPABILITY="$ROOT_DIR/scripts/rebuild_builder_capability.sh"
APPLICABILITY="$ROOT_DIR/scripts/rebuild_builder_applicability.sh"
SERIALIZER_VERIFY="$ROOT_DIR/scripts/verify_fork_serializer.sh"
MEMORY_GUARD="$ROOT_DIR/scripts/memory_guard.sh"
SAFE_REBUILD="$ROOT_DIR/scripts/safe_rebuild.sh"
SERIALIZER_SOURCE="$ROOT_DIR/scripts/fork_serializer.c"
SERIALIZER_SELFTEST_SOURCE="$ROOT_DIR/scripts/fork_serializer_selftest.c"

fail() {
    echo "FAIL: low-task helper serialization: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init low-task-helper-tests) ||
    fail "could not initialize helper-test scope"
TEST_ROOT=$(seen_artifact_mktemp_dir "$test_scope" run) ||
    fail "could not create project-local helper-test root"
[ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] || fail "unsafe helper-test root"
export TMPDIR="$TEST_ROOT"

cleanup() {
    local status=$?
    case "$TEST_ROOT" in
        "$test_scope"/run.*)
            if [ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] &&
                [ "${TEST_ROOT%/*}" = "$test_scope" ]; then

                rm -rf -- "$TEST_ROOT" || status=1
            else
                status=1
            fi
            ;;
        *) status=1 ;;
    esac
    return "$status"
}
trap cleanup EXIT

bash -n "$CAPABILITY" "$APPLICABILITY" "$SERIALIZER_VERIFY" ||
    fail "helper syntax"

guarded_log_body=$(sed -n '/^run_guarded_command_to_log()/,/^}/p' \
    "$SAFE_REBUILD")
grep -Fq 'if [ "${SEEN_REBUILD_AGGREGATE_SCOPE_VERIFIED:-0}" = "1" ]; then' \
    <<< "$guarded_log_body" ||
    fail "aggregate log capture does not select its direct-command path"
grep -Fq '"$@" > "$log_file" 2>&1 || status=$?' <<< "$guarded_log_body" ||
    fail "aggregate log capture retains a persistent logger shell"
grep -Fq 'prepare_bounded_wine_prefix_template || exit $?' "$SAFE_REBUILD" ||
    fail "verify rebuild does not prepare Wine outside the aggregate task topology"
grep -Fq '"$HARD_MEMORY_SCOPE_WRAPPER"' "$SAFE_REBUILD" ||
    fail "Wine prefix preparation lacks a hard-scope entry"

# The acceptance stack leaves very little task headroom. These exact patterns
# previously created three-stage pipelines plus a command-substitution shell.
# Keep all evidence tools serial, with timeout+builder the sole allowed
# two-process pair.
if grep -En '\|[[:space:]]*(awk|sort|grep|sha256sum|tr|wc)([[:space:]]|$)' \
    "$CAPABILITY" "$APPLICABILITY" "$SERIALIZER_VERIFY"; then

    fail "a task-bursting helper pipeline remains"
fi
if grep -En '\$\([^)]*(timeout|readelf|sha256sum|awk|sort|grep|sed|wc|tr)([[:space:]]|\))' \
    "$CAPABILITY" "$APPLICABILITY" "$SERIALIZER_VERIFY"; then

    fail "an external evidence tool remains inside command substitution"
fi
process_substitution_count=0
while IFS= read -r process_substitution; do
    [ -n "$process_substitution" ] || continue
    case "$process_substitution" in
        *"< <(stat -Lc '%d:%i'"*)
            process_substitution_count=$((process_substitution_count + 1))
            ;;
        *) fail "unexpected helper process substitution: $process_substitution" ;;
    esac
done < <(grep -En '< <\(' "$CAPABILITY" "$APPLICABILITY" "$SERIALIZER_VERIFY")
[ "$process_substitution_count" -eq 3 ] ||
    fail "expected exactly three one-child stat identity probes"
grep -Fq 'timeout -k 1 10 "$builder" --help > "$help_file"' "$CAPABILITY" ||
    fail "capability probe is not a synchronous file-backed timeout"
grep -Fq 'readelf -Ws "$compiler" > "$compiler_symbols_file"' "$APPLICABILITY" ||
    fail "compiler symbol probe is not serialized"
grep -Fq 'readelf -Ws "$serializer" > "$serializer_symbols_file"' "$APPLICABILITY" ||
    fail "serializer symbol probe is not serialized"
grep -Fq 'sha256sum -- "$source_file" > "$hash_output_file"' "$SERIALIZER_VERIFY" ||
    fail "serializer digest probe is not serialized"
grep -Fq 'mapfile -t record_lines < "$record"' "$SERIALIZER_VERIFY" ||
    fail "serializer record is not parsed with Bash builtins"

printf '%s\n' '#!/usr/bin/env bash' 'printf "%s\n" "--jobs N --opt-jobs N"' \
    > "$TEST_ROOT/jobs-builder"
printf '%s\n' '#!/usr/bin/env bash' 'printf "%s\n" "--no-fork"' \
    > "$TEST_ROOT/no-fork-builder"
printf '%s\n' '#!/usr/bin/env bash' 'printf "%s\n" "legacy compiler help"' \
    > "$TEST_ROOT/legacy-builder"
chmod 700 "$TEST_ROOT/jobs-builder" "$TEST_ROOT/no-fork-builder" \
    "$TEST_ROOT/legacy-builder"

[ "$(bash "$CAPABILITY" "$TEST_ROOT/jobs-builder")" = "advertised-jobs" ] ||
    fail "jobs capability classification changed"
[ "$(bash "$CAPABILITY" "$TEST_ROOT/no-fork-builder")" = "advertised-no-fork" ] ||
    fail "no-fork capability classification changed"
[ "$(bash "$CAPABILITY" "$TEST_ROOT/legacy-builder")" = "serializer-required" ] ||
    fail "legacy capability classification changed"
if compgen -G "$TEST_ROOT/seen_builder_capability.*.help" >/dev/null; then
    fail "capability probe left project-local scratch"
fi

printf '%s\n' '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'shopt -s nullglob' \
    'probes=("$TMPDIR"/seen_builder_capability.*.help)' \
    '[ "${#probes[@]}" -eq 1 ]' \
    'rm -f -- "${probes[0]}"' \
    'printf "%s\n" "--jobs N --opt-jobs N" > "${probes[0]}"' \
    'printf "%s\n" "--jobs N --opt-jobs N"' \
    > "$TEST_ROOT/replacing-builder"
chmod 700 "$TEST_ROOT/replacing-builder"
set +e
replacement_output=$(bash "$CAPABILITY" "$TEST_ROOT/replacing-builder" \
    2> "$TEST_ROOT/replacing-builder.err")
replacement_status=$?
set -e
[ "$replacement_status" -eq 2 ] ||
    fail "replaced capability probe returned $replacement_status"
[ -z "$replacement_output" ] ||
    fail "replaced capability probe emitted an accepted authorization token"
grep -Fq 'help probe identity changed during classification' \
    "$TEST_ROOT/replacing-builder.err" ||
    fail "replaced capability probe did not fail on identity"
shopt -s nullglob
replacement_probes=("$TEST_ROOT"/seen_builder_capability.*.help)
shopt -u nullglob
[ "${#replacement_probes[@]}" -eq 1 ] && [ -f "${replacement_probes[0]}" ] &&
    [ ! -L "${replacement_probes[0]}" ] ||
    fail "replaced capability probe did not preserve the suspicious target"
rm -f -- "${replacement_probes[0]}"

mock_bin="$TEST_ROOT/mock-bin"
mkdir -p -- "$mock_bin"
printf '%s\n' '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'mode=${1:-}' \
    'target=${2:-}' \
    '[ "${MOCK_READELF_HOLD:-0}" != 1 ] || sleep 0.12' \
    'case "$mode" in' \
    '  -lW) printf "%s\n" "  INTERP         0x000000 0x000000" ;;' \
    '  -Ws)' \
    '    if [[ "$target" == *serializer* ]]; then' \
    '      for symbol in fork posix_spawn posix_spawnp popen pclose waitpid; do' \
    '        printf "1: 0 0 FUNC GLOBAL DEFAULT 12 %s\\n" "$symbol"' \
    '      done' \
    '      if [ "${MOCK_READELF_DROP_WAITPID:-0}" = 1 ]; then' \
    '        exit 0' \
    '      fi' \
    '    else' \
    '      for symbol in fork posix_spawn posix_spawnp popen pclose waitpid; do' \
    '        printf "1: 0 0 FUNC GLOBAL DEFAULT UND %s@GLIBC_2.2.5\\n" "$symbol"' \
    '      done' \
    '      if [ "${MOCK_READELF_UNSUPPORTED:-0}" = 1 ]; then' \
    '        printf "%s\n" "1: 0 0 FUNC GLOBAL DEFAULT UND vfork@GLIBC_2.2.5"' \
    '      fi' \
    '    fi' \
    '    ;;' \
    '  *) exit 64 ;;' \
    'esac' \
    > "$mock_bin/readelf"
chmod 700 "$mock_bin/readelf"
printf '%s\n' '#!/usr/bin/env bash' 'exit 0' > "$TEST_ROOT/mock-compiler"
printf '%s\n' 'serializer fixture' > "$TEST_ROOT/mock-serializer.so"
chmod 700 "$TEST_ROOT/mock-compiler"

PATH="$mock_bin:$PATH" SEEN_MEMORY_GUARD_IN_SCOPE=1 \
    bash "$APPLICABILITY" "$TEST_ROOT/mock-compiler" \
        "$TEST_ROOT/mock-serializer.so" > "$TEST_ROOT/applicable.out" ||
    fail "serialized applicability probe rejected a covered compiler"
[ "$(< "$TEST_ROOT/applicable.out")" = "applicable" ] ||
    fail "serialized applicability probe changed its success classification"
if compgen -G "$TEST_ROOT/seen_builder_applicability.*" >/dev/null; then
    fail "applicability probe left project-local scratch"
fi

set +e
PATH="$mock_bin:$PATH" SEEN_MEMORY_GUARD_IN_SCOPE=1 \
MOCK_READELF_UNSUPPORTED=1 \
    bash "$APPLICABILITY" "$TEST_ROOT/mock-compiler" \
        "$TEST_ROOT/mock-serializer.so" > "$TEST_ROOT/unsupported.out" \
        2> "$TEST_ROOT/unsupported.err"
unsupported_status=$?
set -e
[ "$unsupported_status" -eq 3 ] ||
    fail "unsupported process import returned $unsupported_status"
grep -Fq 'unsupported process/thread creation import: vfork' \
    "$TEST_ROOT/unsupported.err" ||
    fail "unsupported process import classification changed"

# Exercise the real failure topology without a compiler: caller capture ->
# attested-runner shell -> its classify-active capture -> capped-entry shell ->
# applicability helper. Eight inert siblings model the enclosing rebuild and
# guard processes. This userspace TasksMax=16 observer test is intentionally
# not represented as kernel-cgroup proof; the static assertions above are the
# durable one-child helper contract. Do not add these synthetic tasks to a
# real guarded rebuild that is already operating at the same task ceiling.
if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != 1 ]; then
    printf '%s\n' '#!/usr/bin/env bash' \
        'set -euo pipefail' \
        'SEEN_MEMORY_GUARD_IN_SCOPE=1 bash "$1" "$2" "$3"' \
        > "$TEST_ROOT/mock-capped-entry"
    printf '%s\n' '#!/usr/bin/env bash' \
        'set -euo pipefail' \
        'CAPABILITY=$(bash "$1" "${@:2}")' \
        'printf "%s\n" "$CAPABILITY"' \
        > "$TEST_ROOT/mock-attested-runner"
    printf '%s\n' '#!/usr/bin/env bash' \
        'set -euo pipefail' \
        'filler_pids=()' \
        'cleanup_fillers() {' \
        '  local filler_pid=""' \
        '  for filler_pid in "${filler_pids[@]}"; do kill "$filler_pid" 2>/dev/null || true; done' \
        '  for filler_pid in "${filler_pids[@]}"; do wait "$filler_pid" 2>/dev/null || true; done' \
        '}' \
        'trap cleanup_fillers EXIT' \
        'for _ in 1 2 3 4 5 6 7 8; do sleep 10 & filler_pids+=("$!"); done' \
        'captured=$(bash "$1" "${@:2}")' \
        '[ "$captured" = applicable ]' \
        > "$TEST_ROOT/mock-nested-caller"
    chmod 700 "$TEST_ROOT/mock-capped-entry" \
        "$TEST_ROOT/mock-attested-runner" "$TEST_ROOT/mock-nested-caller"
    nested_metrics="$TEST_ROOT/nested-pids16-shape.metrics"
    env -u SEEN_MEMORY_GUARD_IN_SCOPE \
        -u SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE \
        -u SEEN_MEMORY_GUARD_SCOPE_UNIT \
        -u SEEN_MEMORY_GUARD_SCOPE_OWNER \
        SEEN_MEMORY_GUARD_KERNEL_SCOPE=0 \
        MOCK_READELF_HOLD=1 \
        PATH="$mock_bin:$PATH" \
        TMPDIR="$TEST_ROOT" \
        "$MEMORY_GUARD" --tasks-max 16 --timeout-secs 8 \
            --interval-secs 0.02 --kill-only \
            --metrics-file "$nested_metrics" -- \
            "$TEST_ROOT/mock-nested-caller" \
                "$TEST_ROOT/mock-attested-runner" \
                "$TEST_ROOT/mock-capped-entry" \
                "$APPLICABILITY" "$TEST_ROOT/mock-compiler" \
                "$TEST_ROOT/mock-serializer.so" \
            > "$TEST_ROOT/nested.out" 2> "$TEST_ROOT/nested.err" ||
        fail "nested TasksMax=16-shaped helper topology exceeded its budget"
    grep -Fq 'state=complete' "$nested_metrics" ||
        fail "nested helper topology did not complete"
    nested_peak_tasks=""
    while IFS== read -r metric_key metric_value; do
        if [ "$metric_key" = peak_tasks ]; then
            nested_peak_tasks=$metric_value
            break
        fi
    done < "$nested_metrics"
    case "$nested_peak_tasks" in
        ''|*[!0-9]*) fail "nested helper topology recorded no task peak" ;;
    esac
    [ "$nested_peak_tasks" -ge 12 ] && [ "$nested_peak_tasks" -le 16 ] ||
        fail "nested helper topology recorded unexpected peak $nested_peak_tasks"
else
    echo "low-task helper dynamic topology skipped inside active memory guard"
fi

# Build a valid record without compiling anything. It must reach the final
# scope-identity rejection; malformed shape and checksum records must retain
# their earlier fail-closed classifications.
hash_output="$TEST_ROOT/test-hash.out"
hash_file() {
    local source_file=$1
    local destination=$2
    local digest=""
    sha256sum -- "$source_file" > "$hash_output"
    IFS=' ' read -r digest _ < "$hash_output"
    printf -v "$destination" '%s' "$digest"
}
serializer_fixture="$TEST_ROOT/serializer-evidence.so"
record_body="$TEST_ROOT/serializer-record.body"
record_file="$TEST_ROOT/serializer-record.attestation"
printf '%s\n' 'serializer evidence' > "$serializer_fixture"
hash_file "$serializer_fixture" serializer_sha
hash_file "$SERIALIZER_SOURCE" source_sha
hash_file "$SERIALIZER_SELFTEST_SOURCE" selftest_source_sha
fake_scope="seen-memory-guard-999999-999999.scope"
{
    printf 'version=seen-fork-serializer-attestation-v2\n'
    printf 'serializer_sha256=%s\n' "$serializer_sha"
    printf 'source_sha256=%s\n' "$source_sha"
    printf 'selftest_source_sha256=%s\n' "$selftest_source_sha"
    printf 'dynamic_selftest=passed\n'
    printf 'scope_unit=%s\n' "$fake_scope"
    printf 'verified_memory_max_bytes=4294967296\n'
    printf 'verified_memory_swap_max_bytes=0\n'
    printf 'verified_pids_max=16\n'
} > "$record_body"
hash_file "$record_body" record_body_sha
cp -- "$record_body" "$record_file"
printf 'body_sha256=%s\n' "$record_body_sha" >> "$record_file"

set +e
SEEN_MEMORY_GUARD_IN_SCOPE=1 SEEN_MEMORY_GUARD_RSS_KB=4194304 \
    bash "$SERIALIZER_VERIFY" \
    "$serializer_fixture" "$record_file" "$TEST_ROOT" "$fake_scope" \
    > "$TEST_ROOT/verify.out" 2> "$TEST_ROOT/verify.err"
verify_status=$?
set -e
[ "$verify_status" -eq 126 ] || fail "valid mock record returned $verify_status"
grep -Fq 'current process is not inside the recorded scope' "$TEST_ROOT/verify.err" ||
    fail "valid mock record did not reach scope-identity verification"
if compgen -G "$TEST_ROOT/seen_serializer_verify.*" >/dev/null; then
    fail "serializer verifier left project-local scratch"
fi

cp -- "$record_file" "$TEST_ROOT/bad-checksum.attestation"
printf '%064d\n' 0 > "$TEST_ROOT/replacement-hash"
bad_hash=$(< "$TEST_ROOT/replacement-hash")
mapfile -t valid_record_lines < "$record_file"
printf '%s\n' "${valid_record_lines[@]:0:9}" \
    "body_sha256=$bad_hash" > "$TEST_ROOT/bad-checksum.attestation"
set +e
SEEN_MEMORY_GUARD_IN_SCOPE=1 SEEN_MEMORY_GUARD_RSS_KB=4194304 \
    bash "$SERIALIZER_VERIFY" \
    "$serializer_fixture" "$TEST_ROOT/bad-checksum.attestation" \
    "$TEST_ROOT" "$fake_scope" > "$TEST_ROOT/bad-checksum.out" \
    2> "$TEST_ROOT/bad-checksum.err"
bad_checksum_status=$?
set -e
[ "$bad_checksum_status" -eq 126 ] ||
    fail "bad record checksum returned $bad_checksum_status"
grep -Fq 'record checksum mismatch' "$TEST_ROOT/bad-checksum.err" ||
    fail "bad record checksum classification changed"

echo "low-task helper serialization checks passed"
