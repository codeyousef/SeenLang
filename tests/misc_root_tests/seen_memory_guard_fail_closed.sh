#!/usr/bin/env bash

set -u

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GUARD="$ROOT_DIR/scripts/memory_guard.sh"
CAPABILITY="$ROOT_DIR/scripts/rebuild_builder_capability.sh"
APPLICABILITY="$ROOT_DIR/scripts/rebuild_builder_applicability.sh"
SERIALIZER_VERIFY="$ROOT_DIR/scripts/verify_fork_serializer.sh"
BOUNDED_TOOLCHAIN="$ROOT_DIR/scripts/prepare_bounded_toolchain.sh"
FORK_SERIALIZER_SOURCE="$ROOT_DIR/scripts/fork_serializer.c"
FORK_SERIALIZER_SELFTEST="$ROOT_DIR/scripts/fork_serializer_selftest.c"
SAFE_REBUILD="$ROOT_DIR/scripts/safe_rebuild.sh"
HARD_SCOPE="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"
SERIAL_AUXILIARY="$ROOT_DIR/scripts/serial_auxiliary_env.sh"
PACKAGE_BUILD="$ROOT_DIR/scripts/build_package_client.sh"
PREBUILD_GATES="$ROOT_DIR/scripts/seen_prebuild_gates.sh"
RECOVERY_OPT="$ROOT_DIR/scripts/recovery_opt.sh"
SELFHOSTED_ABI_SMOKE="$ROOT_DIR/tests/misc_root_tests/seen_selfhosted_abi_smoke.sh"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
TEST_ROOT=""

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init memory-guard-tests) ||
    fail "could not initialize memory-guard test scope"
TEST_ROOT=$(seen_artifact_mktemp_dir "$test_scope" run) ||
    fail "could not create project-local test directory"
[ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] || fail "unsafe test directory"
export TMPDIR="$TEST_ROOT"

# These checks exercise nested userspace observers while the harness itself
# may already be inside the verified prebuild cgroup. The real outer cgroup
# remains authoritative; clear only inherited markers so mocks cannot be
# mistaken for owners of that aggregate scope.
unset SEEN_MEMORY_GUARD_IN_SCOPE SEEN_MEMORY_GUARD_SCOPE_UNIT
unset SEEN_MEMORY_GUARD_SCOPE_OWNER
unset SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE SEEN_HARD_MEMORY_SCOPE_ACTIVE
OBSERVER_GUARD_ENV=(env
    -u SEEN_MEMORY_GUARD_IN_SCOPE
    -u SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE
    -u SEEN_MEMORY_GUARD_SCOPE_UNIT
    -u SEEN_MEMORY_GUARD_SCOPE_OWNER
    -u SEEN_HARD_MEMORY_SCOPE_ACTIVE
    SEEN_MEMORY_GUARD_KERNEL_SCOPE=0)

cleanup() {
    local status=$?
    if [ "$status" -ne 0 ]; then
        echo "Preserved failed memory-guard test artifacts: $TEST_ROOT" >&2
        return "$status"
    fi
    case "$TEST_ROOT" in
        "$test_scope"/run.*)
            if [ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] &&
                [ "$(dirname -- "$TEST_ROOT")" = "$test_scope" ]; then

                rm -rf -- "$TEST_ROOT"
            fi
            ;;
        *)
            echo "FAIL: refusing to clean unexpected test path: $TEST_ROOT" >&2
            return 1
            ;;
    esac
}
trap cleanup EXIT

bash -n "$GUARD" || fail "memory_guard.sh syntax"
bash -n "$CAPABILITY" || fail "builder capability helper syntax"
bash -n "$APPLICABILITY" || fail "builder applicability helper syntax"
bash -n "$SERIALIZER_VERIFY" || fail "serializer verification helper syntax"
bash -n "$BOUNDED_TOOLCHAIN" || fail "bounded-toolchain helper syntax"
bash -n "$SAFE_REBUILD" || fail "safe_rebuild.sh syntax"
bash -n "$HARD_SCOPE" || fail "hard-memory-scope wrapper syntax"
bash -n "$SERIAL_AUXILIARY" || fail "serial-auxiliary helper syntax"
bash -n "$PACKAGE_BUILD" || fail "package-client build syntax"
bash -n "$PREBUILD_GATES" || fail "prebuild gates syntax"
bash -n "$RECOVERY_OPT" || fail "recovery optimizer syntax"
bash -n "$SELFHOSTED_ABI_SMOKE" || fail "self-hosted ABI smoke syntax"

# The aggregate pids budget is intentionally small. Prove that auxiliary
# runtimes are pinned to one thread and that forged settings fail read-back.
# shellcheck source=scripts/serial_auxiliary_env.sh
source "$SERIAL_AUXILIARY" || fail "could not load serial-auxiliary helper"
seen_serial_auxiliary_prepare "$ROOT_DIR" "$TEST_ROOT" ||
    fail "could not prepare project-local serial auxiliary settings"
seen_serial_auxiliary_verify "$ROOT_DIR" "$TEST_ROOT" ||
    fail "prepared serial auxiliary settings failed verification"
(
    RAYON_NUM_THREADS=2
    export RAYON_NUM_THREADS
    ! seen_serial_auxiliary_verify "$ROOT_DIR" "$TEST_ROOT" >/dev/null 2>&1
) || fail "forged auxiliary thread setting passed verification"
printf '%s\n' '--threads=4' > "$RIPGREP_CONFIG_PATH"
if seen_serial_auxiliary_verify "$ROOT_DIR" "$TEST_ROOT" >/dev/null 2>&1; then
    fail "forged ripgrep configuration passed verification"
fi
seen_serial_auxiliary_prepare "$ROOT_DIR" "$TEST_ROOT" ||
    fail "could not restore serial auxiliary settings"

# A nested rebuild rebinds its validated artifact root after required CI has
# already prepared the outer scope. The fixed policy must be materialized and
# read back at the new root; inheriting the outer path is not evidence.
rebound_auxiliary_root="$TEST_ROOT/rebound-artifacts"
mkdir -p -- "$rebound_auxiliary_root"
seen_serial_auxiliary_prepare "$ROOT_DIR" "$rebound_auxiliary_root" ||
    fail "could not prepare serial auxiliary settings after artifact-root rebind"
seen_serial_auxiliary_verify "$ROOT_DIR" "$rebound_auxiliary_root" ||
    fail "rebound serial auxiliary settings failed verification"
[ "$RIPGREP_CONFIG_PATH" = "$rebound_auxiliary_root/auxiliary-limits/ripgrep.conf" ] ||
    fail "rebound serial auxiliary settings retained the outer configuration path"

supervisor_tmp="$TEST_ROOT/remove-empty-supervisor"
shared_nested_tmp="$TEST_ROOT/shared-nested-tool-tmp"
remove_flag_record="$TEST_ROOT/remove-empty-child.env"
mkdir -p -- "$supervisor_tmp" "$shared_nested_tmp"
SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR=1 \
TMPDIR="$supervisor_tmp" \
    "${OBSERVER_GUARD_ENV[@]}" "$GUARD" -- \
        bash -c '
            printf "%s\n" "${SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR:-missing}" > "$1"
            TMPDIR="$2" "$3" -- true
        ' bash "$remove_flag_record" "$shared_nested_tmp" "$GUARD" ||
    fail "nested remove-empty ownership check failed"
[ "$(cat "$remove_flag_record")" = "0" ] ||
    fail "guarded command inherited supervisor TMPDIR cleanup ownership"
[ ! -e "$supervisor_tmp" ] ||
    fail "owning guard did not remove its empty supervisor TMPDIR"
[ -d "$shared_nested_tmp" ] ||
    fail "nested guard removed the shared project tool TMPDIR"

for option in \
    --rss-limit-kb --available-reserve-kb --vmem-limit-kb --timeout-secs \
    --interval-secs --tasks-max --cgroup-stop-kb --label --metrics-file \
    --success-metrics-file; do
    timeout 2 "$GUARD" "$option" >"$TEST_ROOT/missing.out" 2>"$TEST_ROOT/missing.err"
    status=$?
    [ "$status" -eq 2 ] || fail "$option without a value returned $status"
done

"${OBSERVER_GUARD_ENV[@]}" "$GUARD" --interval-secs 0.10 -- true ||
    fail "0.10 interval was rejected"
for interval in 0 0.00 0.a 1.2 ''; do
    "${OBSERVER_GUARD_ENV[@]}" "$GUARD" --interval-secs "$interval" -- true \
        >"$TEST_ROOT/interval.out" 2>"$TEST_ROOT/interval.err"
    status=$?
    [ "$status" -eq 2 ] || fail "invalid interval '$interval' returned $status"
done

mkdir -p "$TEST_ROOT/mock-bin"
fake_setsid_sentinel="$TEST_ROOT/fake-setsid-ran"
printf '%s\n' '#!/usr/bin/env bash' \
    'touch "'"$fake_setsid_sentinel"'"' \
    'exit 99' \
    > "$TEST_ROOT/mock-bin/setsid"
chmod +x "$TEST_ROOT/mock-bin/setsid"
startup_sentinel="$TEST_ROOT/delayed-start-command-ran"
startup_pid_file="$TEST_ROOT/seen_memory_guard_test_ready_pid.startup"
PATH="$TEST_ROOT/mock-bin:$PATH" \
SEEN_MEMORY_GUARD_TEST_HOOKS=1 \
SEEN_MEMORY_GUARD_TEST_READY_DELAY_SECS=5 \
SEEN_MEMORY_GUARD_TEST_READY_PID_FILE="$startup_pid_file" \
    timeout 3 "${OBSERVER_GUARD_ENV[@]}" "$GUARD" -- \
        sh -c 'touch "$1"' sh "$startup_sentinel" \
        >"$TEST_ROOT/startup.out" 2>"$TEST_ROOT/startup.err"
status=$?
[ "$status" -eq 126 ] || fail "delayed process-group startup returned $status"
[ ! -e "$startup_sentinel" ] || fail "startup timeout ran an unmonitored command"
[ ! -e "$fake_setsid_sentinel" ] || fail "memory guard trusted a PATH-injected setsid"
[ -s "$startup_pid_file" ] || fail "delayed ready hook did not record its isolated root"
startup_pid=$(cat "$startup_pid_file")
case "$startup_pid" in
    ''|*[!0-9]*) fail "delayed ready hook recorded an invalid PID" ;;
esac
startup_cleanup_attempt=0
while kill -0 "$startup_pid" 2>/dev/null &&
    [ "$startup_cleanup_attempt" -lt 100 ]; do

    sleep 0.01
    startup_cleanup_attempt=$((startup_cleanup_attempt + 1))
done
if kill -0 "$startup_pid" 2>/dev/null; then
    kill -KILL "$startup_pid" 2>/dev/null || true
    fail "startup timeout left its trusted-setsid child alive"
fi

observer_marker_sentinel="$TEST_ROOT/inherited-marker-command-ran"
SEEN_MEMORY_GUARD_IN_SCOPE=1 \
SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=1 \
SEEN_MEMORY_GUARD_SCOPE_UNIT="seen-memory-guard-$(id -u)-$$.scope" \
SEEN_MEMORY_GUARD_SCOPE_OWNER=1 \
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1 \
    "${OBSERVER_GUARD_ENV[@]}" "$GUARD" -- \
        sh -c 'touch "$1"' sh "$observer_marker_sentinel" ||
    fail "observer environment did not clear inherited scope markers"
[ -e "$observer_marker_sentinel" ] ||
    fail "observer marker-sanitization command did not run"

missing_scope_sentinel="$TEST_ROOT/missing-scope-command-ran"
SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=1 \
SEEN_MEMORY_GUARD_KERNEL_SCOPE=0 \
    "$GUARD" --rss-limit-kb 1024 --tasks-max 1 -- \
        sh -c 'touch "$1"' sh "$missing_scope_sentinel" \
    >"$TEST_ROOT/missing-scope.out" 2>"$TEST_ROOT/missing-scope.err"
status=$?
[ "$status" -eq 126 ] || fail "disabled required kernel scope returned $status"
[ ! -e "$missing_scope_sentinel" ] || fail "command ran without its required kernel scope"

forged_scope_sentinel="$TEST_ROOT/forged-scope-command-ran"
SEEN_MEMORY_GUARD_IN_SCOPE=1 \
SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=1 \
SEEN_MEMORY_GUARD_KERNEL_SCOPE=1 \
    "$GUARD" --rss-limit-kb 1 --tasks-max 1 -- sh -c 'touch "$1"' sh "$forged_scope_sentinel" \
    >"$TEST_ROOT/forged-scope.out" 2>"$TEST_ROOT/forged-scope.err"
status=$?
[ "$status" -eq 126 ] || fail "forged kernel-scope marker returned $status"
[ ! -e "$forged_scope_sentinel" ] || fail "forged marker bypassed cgroup read-back"

hard_scope_sentinel="$TEST_ROOT/hard-scope-command-ran"
SEEN_MEMORY_GUARD_IN_SCOPE=1 \
SEEN_MEMORY_GUARD_SCOPE_UNIT="seen-memory-guard-$(id -u)-$$.scope" \
SEEN_MEMORY_GUARD_RSS_KB=1024 \
SEEN_MEMORY_GUARD_TASKS_MAX=1 \
    "$HARD_SCOPE" --label "forged direct scope" -- \
        sh -c 'touch "$1"' sh "$hard_scope_sentinel" \
        >"$TEST_ROOT/hard-scope.out" 2>"$TEST_ROOT/hard-scope.err"
status=$?
[ "$status" -eq 126 ] || fail "forged direct hard scope returned $status"
[ ! -e "$hard_scope_sentinel" ] || fail "direct hard-scope wrapper trusted a forged marker"
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1 \
SEEN_MEMORY_GUARD_RSS_KB=1024 \
SEEN_MEMORY_GUARD_TASKS_MAX=1 \
    "$HARD_SCOPE" --label "forged active marker" --verify-only -- \
        >"$TEST_ROOT/hard-scope-active.out" 2>"$TEST_ROOT/hard-scope-active.err"
status=$?
[ "$status" -eq 126 ] || fail "verify-only created a probe scope for a forged marker"

printf '%s\n' '#!/usr/bin/env bash' \
    'echo "$*" >> "'"$TEST_ROOT"'/jobs.calls"' \
    'echo "--jobs N --opt-jobs N"' > "$TEST_ROOT/jobs-builder"
printf '%s\n' '#!/usr/bin/env bash' \
    'echo "$*" >> "'"$TEST_ROOT"'/serial.calls"' \
    'echo "--no-fork"' > "$TEST_ROOT/serial-builder"
printf '%s\n' '#!/usr/bin/env bash' \
    'echo "$*" >> "'"$TEST_ROOT"'/legacy.calls"' \
    'echo "legacy compiler help"' > "$TEST_ROOT/legacy-builder"
chmod +x "$TEST_ROOT/jobs-builder" "$TEST_ROOT/serial-builder" "$TEST_ROOT/legacy-builder"

[ "$(bash "$CAPABILITY" "$TEST_ROOT/jobs-builder")" = "advertised-jobs" ] ||
    fail "jobs builder classification"
[ "$(bash "$CAPABILITY" "$TEST_ROOT/serial-builder")" = "advertised-no-fork" ] ||
    fail "no-fork builder classification"
[ "$(bash "$CAPABILITY" "$TEST_ROOT/legacy-builder")" = "serializer-required" ] ||
    fail "legacy builder was not classified serializer-required"
[ "$(cat "$TEST_ROOT/legacy.calls")" = "--help" ] ||
    fail "legacy capability check invoked anything except --help"

grep -Fq 'SEEN_FORK_SERIALIZER_TARGET' "$FORK_SERIALIZER_SOURCE" ||
    fail "serializer is not bound to an explicit target executable"
grep -Fq 'readlink("/proc/self/exe"' "$FORK_SERIALIZER_SOURCE" ||
    fail "serializer does not verify the current executable identity"
grep -Fq 'serializer_root_pid == getpid()' "$FORK_SERIALIZER_SOURCE" ||
    fail "serializer does not restrict activation to one root PID"
grep -Fq 'descendant_passthrough = 1' "$FORK_SERIALIZER_SOURCE" ||
    fail "fork children do not enter pass-through mode"
grep -Fq 'if (!serializer_is_active() || inside_popen) return real_fork();' \
    "$FORK_SERIALIZER_SOURCE" ||
    fail "libc popen can recursively deadlock through the fork wrapper"
grep -Fq 'target_status.st_dev == current_status.st_dev' \
    "$FORK_SERIALIZER_SOURCE" ||
    fail "serializer target is not bound by executable device/inode"
grep -Fq 'serializer_initialization_status = EACCES' \
    "$FORK_SERIALIZER_SOURCE" ||
    fail "an unproven intended target does not fail closed"
grep -Fq 'pthread_equal(active_popen_owner, pthread_self())' \
    "$FORK_SERIALIZER_SOURCE" ||
    fail "same-thread nested popen creation can deadlock"
grep -Fq 'ACTIVE_CHILD_POISONED' "$FORK_SERIALIZER_SOURCE" ||
    fail "unproven pclose/spawn completion can fail open"
if grep -Fq 'wait_and_save' "$FORK_SERIALIZER_SOURCE"; then
    fail "serializer still synchronously reaps the first fork/spawn"
fi
grep -Fq 'reap_active_pid_locked' "$FORK_SERIALIZER_SOURCE" ||
    fail "serializer does not serialize a later root creation"
grep -Fq 'saved_status_count >=' "$FORK_SERIALIZER_SOURCE" ||
    fail "serializer cache capacity is not checked before reaping"
grep -Fq 'reject_reused_pid_locked(pid)' "$FORK_SERIALIZER_SOURCE" ||
    fail "fork PID reuse can alias an older cached wait status"
grep -Fq 'reject_reused_pid_locked(*pid)' "$FORK_SERIALIZER_SOURCE" ||
    fail "posix_spawn PID reuse can alias an older cached wait status"
grep -Fq 'LARGE_TRANSFER_BYTES = 262144' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks a greater-than-pipe-capacity transfer"
grep -Fq 'run_large_pipeline' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks descendant pipeline coverage"
grep -Fq 'run_large_command_substitution' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks large command-substitution coverage"
grep -Fq 'run_fork_pipe_pressure' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks fork pipe-pressure coverage"
grep -Fq 'run_spawn_pipe_pressure' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks posix_spawn pipe-pressure coverage"
grep -Fq 'run_spawnp_worker' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks posix_spawnp coverage"
grep -Fq 'run_nested_operations' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks nested fork/spawn/popen coverage"
grep -Fq 'test_cached_status_and_two_forks' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks two-fork status-cache coverage"
grep -Fq 'test_wnohang_retains_active' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks WNOHANG coverage"
grep -Fq 'test_stopped_and_signaled_status' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks stopped/signaled status coverage"
grep -Fq 'test_cache_capacity_failure' "$FORK_SERIALIZER_SELFTEST" ||
    fail "serializer self-test lacks cache-capacity failure coverage"
grep -Fq -- '-DSERIALIZER_STATUS_CAPACITY=2' "$SAFE_REBUILD" ||
    fail "serializer cache-capacity test is not built with a bounded seam"
grep -Fq 'fork serializer cache-limit self-test' "$SAFE_REBUILD" ||
    fail "serializer attestation omits cache-capacity failure execution"
grep -Fq 'seen-fork-serializer-attestation-v2' "$SAFE_REBUILD" ||
    fail "serializer behavior change did not bump its attestation version"
grep -Fq 'seen-fork-serializer-attestation-v2' "$SERIALIZER_VERIFY" ||
    fail "serializer verifier does not require the v2 attestation"
grep -Fq 'recorded memory.max differs from the active cgroup read-back' \
    "$SERIALIZER_VERIFY" ||
    fail "serializer attestation is not bound to active memory.max"
grep -Fq 'recorded memory.swap.max differs from the active cgroup read-back' \
    "$SERIALIZER_VERIFY" ||
    fail "serializer attestation is not bound to active memory.swap.max"
grep -Fq 'recorded pids.max differs from the active cgroup read-back' \
    "$SERIALIZER_VERIFY" ||
    fail "serializer attestation is not bound to active pids.max"
grep -Fq 'active cgroup memory.max exceeds the current-memory-derived cap' \
    "$SERIALIZER_VERIFY" ||
    fail "active serializer scope is not checked against the derived cap"
grep -Fq 'total_ceiling_kb=$((total_kb * 60 / 100))' "$HARD_SCOPE" ||
    fail "nested scope read-back lacks a stable total-memory ceiling"
grep -Fq 'if [ "$VERIFY_ONLY" = "0" ] && [ "$derived_rss_kb" -gt "$available_cap_kb" ]' \
    "$HARD_SCOPE" ||
    fail "scope creation no longer derives its cap from current availability"
grep -Fq 'fork serializer target rejection self-test' "$SAFE_REBUILD" ||
    fail "serializer attestation omits mismatched-target rejection"

scope_line=$(grep -n 'enter_rebuild_kernel_scope$' "$SAFE_REBUILD" | tail -1 | cut -d: -f1)
package_line=$(grep -n 'prepare_package_client || exit 1' "$SAFE_REBUILD" | tail -1 | cut -d: -f1)
serializer_line=$(grep -n 'build_fork_serializer || exit 1' "$SAFE_REBUILD" | tail -1 | cut -d: -f1)
bounded_line=$(grep -n 'prepare_bounded_toolchain || exit 1' "$SAFE_REBUILD" | tail -1 | cut -d: -f1)
case "$scope_line:$package_line:$serializer_line:$bounded_line" in
    *[!0-9:]*|:*|*:) fail "could not locate aggregate-scope ordering guards" ;;
esac
[ "$scope_line" -lt "$package_line" ] || fail "package client can run before aggregate scope"
[ "$scope_line" -lt "$serializer_line" ] || fail "fork serializer can run before aggregate scope"
[ "$serializer_line" -lt "$bounded_line" ] || fail "bounded toolchain can run before serializer attestation"
[ "$bounded_line" -lt "$package_line" ] || fail "helper/compiler work can run before bounded toolchain setup"
grep -Fq 'run_guarded_command "package client build"' "$SAFE_REBUILD" ||
    fail "package-client helper build is not secondarily guarded"
grep -Fq 'MemorySwapMax=0' "$GUARD" || fail "MemorySwapMax request missing"
grep -Fq 'tasks_max="${TASKS_MAX:-24}"' "$GUARD" ||
    fail "kernel scope can still default above the 24-task ceiling"
grep -Fq '[ "$memory_swap_max" != "0" ]' "$GUARD" ||
    fail "MemorySwapMax read-back missing"
grep -Fq 'memory.oom.group' "$GUARD" ||
    fail "group-wide OOM control is missing"
grep -Fq '[ "$memory_oom_group" != "1" ]' "$GUARD" ||
    fail "group-wide OOM read-back is missing"
if grep -Fq 'OOMPolicy=kill' "$GUARD"; then
    fail "memory guard still depends on version-specific OOMPolicy"
fi
if grep -Fq 'TMPDIR:-/tmp' "$GUARD"; then
    fail "memory guard can fall back to the host temporary directory"
fi
if grep -Fq 'command -v setsid' "$GUARD" || grep -Fq 'exec setsid ' "$GUARD"; then
    fail "memory guard can resolve setsid through caller-controlled PATH"
fi
grep -Fq 'for setsid_candidate in /usr/bin/setsid /bin/setsid' "$GUARD" ||
    fail "memory guard does not resolve a trusted absolute setsid"
grep -Fq 'exec "$TRUSTED_SETSID" bash -c' "$GUARD" ||
    fail "guarded command does not use the trusted setsid path"
token_export_line=$(grep -n -m1 'export SEEN_MEMORY_GUARD_PROCESS_TOKENS=' \
    "$GUARD" | cut -d: -f1)
setsid_exec_line=$(grep -n -m1 'exec "$TRUSTED_SETSID" bash -c' \
    "$GUARD" | cut -d: -f1)
case "$token_export_line:$setsid_exec_line" in
    *[!0-9:]*) fail "could not prove pre-setsid process-token ordering" ;;
esac
[ "$token_export_line" -lt "$setsid_exec_line" ] ||
    fail "process token is not exported before trusted setsid"
grep -Fq 'SEEN_MEMORY_GUARD_TEST_READY_DELAY_SECS' "$GUARD" ||
    fail "post-setsid ready-delay regression hook is missing"
grep -Fq 'export SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR=0' "$GUARD" ||
    fail "guarded commands inherit supervisor TMPDIR cleanup ownership"
grep -Fq 'SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR=0' "$SAFE_REBUILD" ||
    fail "safe rebuild does not revoke nested TMPDIR cleanup ownership"
grep -Fq 'SEEN_MEMORY_GUARD_SCOPE_OWNER=1' "$GUARD" ||
    fail "outer scope-owner marker missing"
grep -Fq 'is_positive_integer "$unit_pid" || return 1' "$GUARD" ||
    fail "scope-unit PID identity is not strictly numeric"
grep -Fq 'export SEEN_MEMORY_GUARD_SCOPE_OWNER=0' "$GUARD" ||
    fail "scope ownership is inherited by nested guards"
grep -Fq 'systemctl --user kill --signal=KILL --kill-whom=all "$scope_unit"' "$GUARD" ||
    fail "cgroup-wide stop missing"
grep -Fq 'owner_scope_remaining_processes()' "$GUARD" ||
    fail "outer guard cannot prove its cgroup is empty after root exit"
grep -Fq 'done < "$VERIFIED_CGROUP_DIR/cgroup.procs"' "$GUARD" ||
    fail "outer guard does not inspect remaining cgroup processes"
[ "$(grep -Fc 'stop_process_group "$pgid" "$urgent"' "$GUARD")" -eq 1 ] ||
    fail "a resource stop bypasses stop_guarded_tree"
[ "$(grep -Fc 'stop_guarded_tree "$pgid"' "$GUARD")" -ge 6 ] ||
    fail "not every resource stop uses tree containment"
grep -Fq 'verified_memory_max_bytes=' "$GUARD" ||
    fail "actual cgroup read-back is absent from metrics"
grep -Fq 'metrics_version=4' "$GUARD" ||
    fail "native cgroup accounting did not bump the canonical metrics schema"
for native_metric in \
    cgroup_memory_current_bytes cgroup_memory_peak_bytes \
    cgroup_memory_events_oom cgroup_memory_events_oom_kill \
    cgroup_pids_current cgroup_pids_peak cgroup_pids_events_max; do

    grep -Fq "${native_metric}=" "$GUARD" ||
        fail "canonical metrics omit $native_metric"
done
grep -Fq 'summed_rss_role=telemetry_only' "$GUARD" ||
    fail "verified cgroup metrics do not mark summed RSS as telemetry"
grep -Fq '[ "$kernel_cgroup_mode" != "1" ] &&' "$GUARD" ||
    fail "summed RSS can still stop a verified cgroup"
grep -Fq 'read_cgroup_process_snapshot "$cgroup_dir"' "$GUARD" ||
    fail "verified cgroup polling lacks builtin process telemetry"
grep -Fq 'capture_native_cgroup_metrics "$cgroup_dir"' "$GUARD" ||
    fail "verified cgroup polling does not capture native accounting"
kernel_poll_start=$(grep -n -m1 '\[ "$authoritative_cgroup_monitor" = "1" \]; then' \
    "$GUARD" | cut -d: -f1)
observer_poll_start=$(grep -n -m1 'snapshot=$(process_group_snapshot' \
    "$GUARD" | cut -d: -f1)
case "$kernel_poll_start:$observer_poll_start" in
    *[!0-9:]*) fail "could not prove verified-cgroup polling split" ;;
esac
[ "$kernel_poll_start" -lt "$observer_poll_start" ] ||
    fail "verified cgroup polling falls through to ps/awk snapshotting"
for terminal_state in tasks_limit rss_limit cgroup_limit; do
    grep -Fq "write_metrics_file \"$terminal_state\" \"\$peak_rss\" \"\$peak_cgroup_kb\" \"\$rss\" \"\$peak_tasks\" 137" \
        "$GUARD" || fail "$terminal_state does not publish status with terminal metrics"
done
if grep -Fq '>> "$METRICS_FILE"' "$GUARD"; then
    fail "metrics status finalization is not atomic"
fi
grep -Fq 'mv -f -- "$temporary" "$METRICS_FILE"' "$GUARD" ||
    fail "canonical metrics are not installed atomically"
grep -Fq -- '--success-metrics-file "$successful_metrics"' "$SAFE_REBUILD" ||
    fail "safe rebuild does not retain successful aggregate evidence"
grep -Fq 'TMPDIR="$supervisor_tmp"' "$SAFE_REBUILD" ||
    fail "outer guard state does not use its reserved supervisor directory"
grep -Fq '[ "$cleanup_entry" != "$supervisor_dir" ] || continue' "$SAFE_REBUILD" ||
    fail "child success cleanup can delete live outer-guard state"
grep -Fq 'SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR=1' "$SAFE_REBUILD" ||
    fail "outer supervisor directory is not cleaned after final retention"
grep -Fq 'kill_scope_matching_processes()' "$SAFE_REBUILD" ||
    fail "rebuild cleanup is not cgroup-scoped"
if grep -Fq 'kill_matching_processes()' "$SAFE_REBUILD"; then
    fail "global pattern-based process cleanup remains"
fi
grep -Fq 'candidate_cgroup" = "$current_cgroup' "$SAFE_REBUILD" ||
    fail "process cleanup does not compare the candidate cgroup"
grep -Fq 'SEEN_TRACE_BUILD="$REBUILD_WORK_ROOT/safe-rebuild.trace.jsonl"' "$SAFE_REBUILD" ||
    fail "safe rebuild trace can escape the project artifact root"
if grep -Fq 'check_cmd+=(' "$SAFE_REBUILD"; then
    fail "strict check command still receives compile-only worker flags"
fi
grep -Fq 'aborting the rebuild without fallback or retry' "$SAFE_REBUILD" ||
    fail "resource guard stops do not abort the rebuild"
grep -Fq 'REBUILD_FATAL_STATUS_FILE="$REBUILD_WORK_ROOT/.fatal-containment-status"' \
    "$SAFE_REBUILD" || fail "fatal containment status is not project-local"
grep -Fq 'record_rebuild_fatal_status "$guarded_status"' "$SAFE_REBUILD" ||
    fail "guard resource status is not recorded before terminating"
grep -Fq 'record_rebuild_fatal_status "$status"' "$SAFE_REBUILD" ||
    fail "diagnostic resource status is not recorded before terminating"
grep -Fq 'trap safe_rebuild_handle_term TERM' "$SAFE_REBUILD" ||
    fail "TERM does not preserve the recorded containment status"
grep -Fq 'read_rebuild_fatal_status 2>/dev/null || true' "$SAFE_REBUILD" ||
    fail "EXIT cleanup does not defend against a recorded fatal status"
grep -Fq 'guard_state="resource_diagnostic"' "$SAFE_REBUILD" ||
    fail "nonzero allocation diagnostics are not classified as resource stops"
grep -Fq 'cannot allocate memory|could not allocate memory' "$SAFE_REBUILD" ||
    fail "allocation-failure log classification is incomplete"
grep -Fq 'resource stop:|out of memory' "$SAFE_REBUILD" ||
    fail "explicit RESOURCE STOP diagnostics are not fatal"
grep -Fq 'resource temporarily unavailable|cannot fork' "$SAFE_REBUILD" ||
    fail "task-creation failures are not fatal"
if grep -A12 -F 'start_log_failure_watcher()' "$SAFE_REBUILD" |
    grep -Eq 'kill -(TERM|KILL) "\$watched_pid"'; then
    fail "failure watcher can orphan the authenticated guard/compiler tree"
fi
disabled_failure_watcher=$(sed -n '/^start_log_failure_watcher()/,/^}/p' \
    "$SAFE_REBUILD")
if grep -Eq '^[[:space:]]*(grep|sleep)[[:space:]]|\) &' \
    <<< "$disabled_failure_watcher"; then
    fail "disabled failure watcher still consumes task slots"
fi
guarded_log_wrapper=$(sed -n '/^run_guarded_command_to_log_with_failure_watch()/,/^}/p' \
    "$SAFE_REBUILD")
if grep -Fq ' &' <<< "$guarded_log_wrapper" ||
    grep -Fq 'wait ' <<< "$guarded_log_wrapper"; then

    fail "guarded log wrapper still backgrounds synchronous compiler work"
fi
if grep -Eq 'OOM kills are non-deterministic|re-running frozen compiler to fill' "$SAFE_REBUILD"; then
    fail "resource-sensitive frozen compiler retry remains"
fi
for direct_script in \
    "$ROOT_DIR/scripts/build_package_client.sh" \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" \
    "$ROOT_DIR/scripts/seen_prebuild_gates.sh" \
    "$ROOT_DIR/scripts/run_all_tests.sh" \
    "$ROOT_DIR/tests/e2e_multilang/run_all_e2e.sh"; do

    grep -Fq 'run_in_hard_memory_scope.sh' "$direct_script" ||
        fail "build-capable entry point lacks a hard-scope wrapper: $direct_script"
    grep -Fq -- '--verify-only --' "$direct_script" ||
        fail "build-capable entry point lacks actual scope read-back: $direct_script"
done
for cache_variable in GOCACHE GOMODCACHE GOPATH GOTMPDIR; do
    grep -Fq "$cache_variable=\"\$PACKAGE_CACHE_ROOT/" "$PACKAGE_BUILD" ||
        fail "package-client $cache_variable is not project-local"
done
grep -Fq 'GOMAXPROCS=1 GOFLAGS="-p=1 -modcacherw"' "$PACKAGE_BUILD" ||
    fail "package-client build fan-out is not serial or its cache is not cleanup-safe"
grep -Fq 'max_early_stop_kb=$MEMORY_GUARD_RSS_KB' \
    "$SAFE_REBUILD" ||
    fail "safe rebuild stop watermark does not use the full bounded hard cap"
grep -Fq 'high_kb=$((RSS_LIMIT_KB * 99 / 100))' "$GUARD" ||
    fail "kernel MemoryHigh does not match the bounded proactive watermark"
grep -Fq 'active_vmem_kb=$(ulimit -S -v' "$PACKAGE_BUILD" ||
    fail "package-client fallback cap is not read back"
grep -Fq '[ "$active_vmem_kb" -gt "$VMEM_KB" ]' "$PACKAGE_BUILD" ||
    fail "package-client fallback cap read-back is not bounded"
if grep -Fq 'ulimit -v "$OPT_VMEM_KB" 2>/dev/null || true' "$PREBUILD_GATES"; then
    fail "prebuild optimizer cap can fail open"
fi
grep -Fq 'if ! ulimit -S -v "$OPT_VMEM_KB"' "$PREBUILD_GATES" ||
    fail "prebuild optimizer cap failure is not fatal"
grep -Fq '[ "$active_opt_vmem" -gt "$OPT_VMEM_KB" ]' "$PREBUILD_GATES" ||
    fail "prebuild optimizer cap is not read back"
if grep -Fq 'ulimit -v "\$SEEN_OPT_VMEM_KB" 2>/dev/null || true' "$SAFE_REBUILD"; then
    fail "generated rebuild optimizer wrapper can fail open"
fi
grep -Fq 'if ! ulimit -S -v "\$SEEN_OPT_VMEM_KB"' "$SAFE_REBUILD" ||
    fail "generated rebuild optimizer wrapper does not fail closed"
grep -Fq 'ACTIVE_OPT_VMEM=\$(ulimit -S -v' "$SAFE_REBUILD" ||
    fail "generated rebuild optimizer cap is not read back"
if grep -Fq 'ulimit -v "$SEEN_OPT_VMEM_KB" 2>/dev/null || true' "$RECOVERY_OPT"; then
    fail "recovery optimizer cap can fail open"
fi
grep -Fq 'if ! ulimit -S -v "$SEEN_OPT_VMEM_KB"' "$RECOVERY_OPT" ||
    fail "recovery optimizer does not fail closed when setting its cap"
grep -Fq '[ "$active_opt_vmem" -gt "$SEEN_OPT_VMEM_KB" ]' "$RECOVERY_OPT" ||
    fail "recovery optimizer cap is not read back"
grep -Fq 'standalone recovery is unsupported' "$RECOVERY_OPT" ||
    fail "standalone recovery does not fail closed before creating output"
grep -Fq 'SEEN_REBUILD_AGGREGATE_SCOPE_ACTIVE' "$RECOVERY_OPT" ||
    fail "recovery output is not bound to its owning rebuild"
grep -Fq 'run_in_hard_memory_scope.sh' "$RECOVERY_OPT" ||
    fail "standalone recovery does not enter a hard aggregate scope"
grep -Fq -- '--verify-only --' "$RECOVERY_OPT" ||
    fail "recovery optimizer does not read back its aggregate scope"
if grep -Eq 'SEEN_RECOVERY_LL_DIR:-/tmp|mktemp -d /tmp/' "$RECOVERY_OPT"; then
    fail "recovery optimizer can use the host temporary directory"
fi
if grep -Fq 'ulimit -v "$VMEM_KB" 2>/dev/null || true' "$SELFHOSTED_ABI_SMOKE"; then
    fail "self-hosted ABI smoke cap can fail open"
fi
grep -Fq 'if ! ulimit -S -v "$VMEM_KB"' "$SELFHOSTED_ABI_SMOKE" ||
    fail "self-hosted ABI smoke does not fail closed when setting its cap"
grep -Fq '[ "$active_vmem" -gt "$VMEM_KB" ]' "$SELFHOSTED_ABI_SMOKE" ||
    fail "self-hosted ABI smoke cap is not read back"
grep -Fq 'exit "$opt_status"' "$RECOVERY_OPT" ||
    fail "recovery optimizer does not preserve resource-stop status"
grep -Fq 'exit "$(normalized_failure_status "$compile_status")"' "$SELFHOSTED_ABI_SMOKE" ||
    fail "self-hosted ABI smoke erases compiler resource-stop status"
for serialized_symbol in fork posix_spawn posix_spawnp popen pclose waitpid; do
    grep -Eq "(^|[[:space:]*])${serialized_symbol}\\(" "$FORK_SERIALIZER_SOURCE" ||
        fail "fork serializer does not interpose $serialized_symbol"
done
for helper_tool in opt llvm-as llvm-link llc clang ld ld.lld lld glslc; do
    grep -Fq "$helper_tool" "$BOUNDED_TOOLCHAIN" ||
        fail "bounded toolchain omits $helper_tool"
done
grep -Fq 'could not apply optimizer/helper virtual-memory cap' "$BOUNDED_TOOLCHAIN" ||
    fail "bounded toolchain wrappers do not fail closed on cap setup"
grep -Fq 'active_kb=$(ulimit -S -v' "$BOUNDED_TOOLCHAIN" ||
    fail "bounded toolchain wrappers do not read back their cap"
[ "$(grep -Fc 'compiler_serializer_applicable' "$SAFE_REBUILD")" -ge 6 ] ||
    fail "not every compiler-builder boundary checks serializer applicability"
grep -Fq 'safe_rebuild_validate_install_destinations || exit 1' "$SAFE_REBUILD" ||
    fail "safe rebuild does not validate install destinations before compiler use"
grep -Fq 'safe_rebuild_assert_checkout_output "$relative_path"' "$SAFE_REBUILD" ||
    fail "safe rebuild install endpoints are not checked"
grep -Fq 'safe_rebuild_install_checkout_file "$VERIFIED"' "$SAFE_REBUILD" ||
    fail "full rebuild install bypasses atomic safe installation"
grep -Fq 'SEEN_DEFER_SELFHOSTED_ABI_SMOKE=1' "$SAFE_REBUILD" ||
    fail "clean full rebuild does not defer ABI smoke until a fresh candidate exists"
grep -Fq 'SEEN_SELFHOSTED_ABI_COMPILER="$REAL_COMPILER"' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "fresh-candidate acceptance does not bind the self-hosted ABI smoke"
grep -Fq 'SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"' "$SELFHOSTED_ABI_SMOKE" ||
    fail "self-hosted ABI smoke does not bind the compiler source root"
grep -Fq 'cd "$PROJECT_DIR"' "$SELFHOSTED_ABI_SMOKE" ||
    fail "self-hosted ABI smoke does not preserve project module resolution"
grep -Fq 'compile main.seen "$OUTPUT_FILE" --fast --no-cache' \
    "$SELFHOSTED_ABI_SMOKE" ||
    fail "self-hosted ABI smoke does not compile the project entry"
if grep -Fq 'eval ' "$ROOT_DIR/scripts/run_all_tests.sh"; then
    fail "legacy all-tests still executes interpolated compiler commands with eval"
fi
for direct_compile_script in \
    "$ROOT_DIR/scripts/run_all_tests.sh" \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" \
    "$ROOT_DIR/tests/e2e_multilang/run_all_e2e.sh"; do
    grep -Fq 'verify_fork_serializer.sh' "$direct_compile_script" ||
        fail "direct compiler entry does not verify serializer attestation: $direct_compile_script"
    grep -Fq 'prepare_bounded_toolchain.sh' "$direct_compile_script" ||
        fail "direct compiler entry does not install bounded helper wrappers: $direct_compile_script"
    grep -Fq 'LD_PRELOAD' "$direct_compile_script" ||
        fail "direct compiler entry does not apply the serializer: $direct_compile_script"
    grep -Fq 'SEEN_FORK_SERIALIZER_TARGET' "$direct_compile_script" ||
        fail "direct compiler entry does not bind the serializer target: $direct_compile_script"
done
grep -Fq '[ "$MAIN_COMPILER_VMEM_KB" -gt "$DERIVED_MEMORY_CAP_KB" ]' "$SAFE_REBUILD" ||
    fail "safe rebuild accepts a compiler cap above the current-memory-derived limit"
grep -Fq '[ "$MEMORY_GUARD_RSS_KB" -gt "$DERIVED_MEMORY_CAP_KB" ]' "$SAFE_REBUILD" ||
    fail "safe rebuild accepts an aggregate guard cap above the current-memory-derived limit"
grep -Fq '[ "$OPT_VMEM_KB" -gt 2097152 ]' "$SAFE_REBUILD" ||
    fail "safe rebuild lacks the 2 GiB optimizer hard maximum"
grep -Fq '[ "$MAIN_COMPILER_MEMORY_LIMIT_BYTES" -gt "$((MAIN_COMPILER_VMEM_KB * 1024))" ]' "$SAFE_REBUILD" ||
    fail "safe rebuild accepts an allocation budget above the hard aggregate cap"
grep -Fq '[[ "$VMEM_KB" -gt 2097152 ]]' "$PACKAGE_BUILD" ||
    fail "package-client helper accepts a cap above 2 GiB"
grep -Fq 'unset SEEN_MEMORY_GUARD_METRICS_FILE SEEN_MEMORY_GUARD_SUCCESS_METRICS_FILE' "$HARD_SCOPE" ||
    fail "direct hard-scope wrapper forwards arbitrary metrics paths"
grep -Fq '[ "${SEEN_JOBS:-0}" != "1" ]' "$HARD_SCOPE" ||
    fail "verify-only does not require serial compiler workers"
grep -Fq '[ "$SEEN_OPT_VMEM_KB" -gt 2097152 ]' "$HARD_SCOPE" ||
    fail "verify-only does not recheck the 2 GiB optimizer ceiling"
grep -Fq 'seen_serial_auxiliary_verify "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT"' \
    "$HARD_SCOPE" || fail "verify-only does not revalidate auxiliary thread settings"
grep -Fq 'seen_serial_auxiliary_prepare "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT"' \
    "$HARD_SCOPE" || fail "hard-scope entry does not prepare auxiliary thread settings"
grep -Fq 'seen_serial_auxiliary_prepare "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT"' \
    "$SAFE_REBUILD" || fail "safe rebuild does not prepare auxiliary thread settings"
grep -Fq 'seen_serial_auxiliary_verify "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT"' \
    "$SAFE_REBUILD" || fail "scoped safe rebuild does not verify auxiliary thread settings"
grep -Fq "printf '%s\\n' '--threads=1'" "$SERIAL_AUXILIARY" ||
    fail "project-local ripgrep single-thread configuration is missing"
for auxiliary_variable in \
    RAYON_NUM_THREADS OMP_NUM_THREADS OPENBLAS_NUM_THREADS \
    MKL_NUM_THREADS NUMEXPR_NUM_THREADS VECLIB_MAXIMUM_THREADS \
    BLIS_NUM_THREADS GOMAXPROCS RUST_TEST_THREADS CARGO_BUILD_JOBS \
    SEEN_LLD_THREADS SEEN_THINLTO_JOBS; do

    grep -Fq "$auxiliary_variable=1" "$SERIAL_AUXILIARY" ||
        fail "$auxiliary_variable is not pinned to one"
done
grep -Fq -- '--threads=1 --thinlto-jobs=1' "$BOUNDED_TOOLCHAIN" ||
    fail "LLD wrapper does not force one thread/ThinLTO job"
grep -Fq -- '-flto-jobs=1' "$BOUNDED_TOOLCHAIN" ||
    fail "clang ThinLTO wrapper does not force one backend job"
(
    export SEEN_ARTIFACT_ROOT="$TEST_ROOT"
    export SEEN_MEMORY_GUARD_IN_SCOPE=1
    export SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
    export SEEN_OPT_VMEM_KB=524288
    "$BOUNDED_TOOLCHAIN" "$TEST_ROOT" > "$TEST_ROOT/bounded-toolchain.path"
) || fail "could not generate bounded wrappers for syntax inspection"
generated_toolchain=$(cat "$TEST_ROOT/bounded-toolchain.path")
[ "$generated_toolchain" = "$TEST_ROOT/bounded-toolchain" ] ||
    fail "bounded-toolchain generator returned an unexpected path"
for generated_wrapper in "$generated_toolchain"/*; do
    [ -f "$generated_wrapper" ] || continue
    bash -n "$generated_wrapper" ||
        fail "generated bounded wrapper has invalid syntax: $generated_wrapper"
done
if [ -f "$generated_toolchain/ld.lld" ]; then
    grep -Fq -- '--threads=1 --thinlto-jobs=1' \
        "$generated_toolchain/ld.lld" ||
        fail "generated ld.lld wrapper omits one-thread flags"
fi
if [ -f "$generated_toolchain/clang" ]; then
    grep -Fq -- '-flto-jobs=1' "$generated_toolchain/clang" ||
        fail "generated clang wrapper omits one-job ThinLTO flag"
fi
prebuild_readback_line=$(grep -n -m1 -- '--verify-only --' "$PREBUILD_GATES" | cut -d: -f1)
prebuild_rg_line=$(grep -n -m1 'if rg -n' "$PREBUILD_GATES" | cut -d: -f1)
case "$prebuild_readback_line:$prebuild_rg_line" in
    *[!0-9:]*) fail "could not prove prebuild ripgrep scope ordering" ;;
esac
[ "$prebuild_readback_line" -lt "$prebuild_rg_line" ] ||
    fail "prebuild invokes ripgrep before hard-scope read-back"

success_metrics="$TEST_ROOT/success.metrics"
last_success_metrics="$TEST_ROOT/last-success.metrics"
"${OBSERVER_GUARD_ENV[@]}" "$GUARD" --metrics-file "$success_metrics" \
        --success-metrics-file "$last_success_metrics" -- true ||
    fail "successful metrics retention failed"
grep -Fq 'command_status=0' "$last_success_metrics" ||
    fail "retained success metrics lack final command status"

task_pid_file="$TEST_ROOT/task-child.pid"
task_metrics="$TEST_ROOT/task-limit.metrics"
"${OBSERVER_GUARD_ENV[@]}" "$GUARD" \
        --rss-limit-kb 1048576 --tasks-max 1 --interval-secs 0.05 \
        --kill-only --metrics-file "$task_metrics" -- \
        bash -c 'sleep 30 & echo $! > "$1"; wait' bash "$task_pid_file" \
        >"$TEST_ROOT/task-limit.out" 2>"$TEST_ROOT/task-limit.err"
status=$?
[ "$status" -eq 137 ] || fail "process-group task stop returned $status"
grep -Fq 'state=tasks_limit' "$task_metrics" || fail "task limit metrics missing"
[ "$(grep -Fc 'command_status=137' "$task_metrics")" -eq 1 ] ||
    fail "task limit metrics lack one exact terminal status"
if [ -s "$task_pid_file" ]; then
    task_pid=$(cat "$task_pid_file")
    if kill -0 "$task_pid" 2>/dev/null; then
        kill -KILL "$task_pid" 2>/dev/null || true
        fail "process-group stop left a child alive"
    fi
fi

mock_cgroup_dir="$TEST_ROOT/seen_memory_guard_test_cgroup.primary"
cgroup_metrics="$TEST_ROOT/cgroup-limit.metrics"
cgroup_torn_tmp="$cgroup_metrics.tmp.interrupted"
mkdir -p -- "$mock_cgroup_dir"
printf '%s\n' '4096' > "$mock_cgroup_dir/memory.current"
printf '%s\n' '8192' > "$mock_cgroup_dir/memory.peak"
printf '%s\n' 'low 0' 'high 1' 'max 2' 'oom 0' 'oom_kill 0' \
    'oom_group_kill 0' > "$mock_cgroup_dir/memory.events"
printf '%s\n' 'low 0' 'high 1' 'max 2' 'oom 0' 'oom_kill 0' \
    'oom_group_kill 0' > "$mock_cgroup_dir/memory.events.local"
printf '%s\n' '4' > "$mock_cgroup_dir/pids.current"
printf '%s\n' '7' > "$mock_cgroup_dir/pids.peak"
printf '%s\n' 'max 3' > "$mock_cgroup_dir/pids.events"
printf '%s\n' 'max 3' > "$mock_cgroup_dir/pids.events.local"
: > "$mock_cgroup_dir/cgroup.procs"
printf '%s\n' 'metrics_version=4' > "$cgroup_torn_tmp"
mock_hook_sentinel="$TEST_ROOT/mock-cgroup-hook-command-ran"
SEEN_MEMORY_GUARD_IN_SCOPE=1 \
SEEN_MEMORY_GUARD_SCOPE_OWNER=0 \
SEEN_MEMORY_GUARD_KERNEL_SCOPE=0 \
SEEN_MEMORY_GUARD_TEST_CGROUP_DIR="$mock_cgroup_dir" \
    "$GUARD" --rss-limit-kb 1048576 --cgroup-stop-kb 1 -- \
        sh -c 'touch "$1"' sh "$mock_hook_sentinel" \
        >"$TEST_ROOT/cgroup-hook-rejected.out" \
        2>"$TEST_ROOT/cgroup-hook-rejected.err"
status=$?
[ "$status" -eq 126 ] || fail "ungated cgroup-memory test hook returned $status"
[ ! -e "$mock_hook_sentinel" ] || fail "ungated cgroup-memory test hook ran a command"
SEEN_MEMORY_GUARD_IN_SCOPE=1 \
SEEN_MEMORY_GUARD_SCOPE_OWNER=0 \
SEEN_MEMORY_GUARD_KERNEL_SCOPE=0 \
SEEN_MEMORY_GUARD_TEST_HOOKS=1 \
SEEN_MEMORY_GUARD_TEST_CGROUP_DIR="$mock_cgroup_dir" \
    "$GUARD" --rss-limit-kb 1048576 --cgroup-stop-kb 1 \
        --interval-secs 0.05 --kill-only --metrics-file "$cgroup_metrics" -- \
        sleep 30 >"$TEST_ROOT/cgroup-limit.out" 2>"$TEST_ROOT/cgroup-limit.err"
status=$?
[ "$status" -eq 137 ] || fail "mock cgroup stop returned $status"
[ "$(sed -n '1p' "$cgroup_metrics")" = "metrics_version=4" ] ||
    fail "mock cgroup stop left no complete canonical metrics record"
grep -Fq 'state=cgroup_limit' "$cgroup_metrics" ||
    fail "mock cgroup stop metrics lack terminal state"
[ "$(grep -Fc 'command_status=137' "$cgroup_metrics")" -eq 1 ] ||
    fail "mock cgroup stop metrics lack one exact terminal status"
grep -Fq 'recorded_at=' "$cgroup_metrics" ||
    fail "mock cgroup stop metrics were torn before atomic installation"
[ "$(cat "$cgroup_torn_tmp")" = "metrics_version=4" ] ||
    fail "canonical metrics publication unexpectedly reused an unrelated temporary file"
grep -Fq 'cgroup_memory_current_bytes=4096' "$cgroup_metrics" ||
    fail "mock cgroup stop did not persist memory.current"
grep -Fq 'cgroup_memory_peak_bytes=8192' "$cgroup_metrics" ||
    fail "mock cgroup stop did not persist memory.peak"
grep -Fq 'cgroup_memory_events_high=1' "$cgroup_metrics" ||
    fail "mock cgroup stop did not persist memory.events"
grep -Fq 'cgroup_pids_current=4' "$cgroup_metrics" ||
    fail "mock cgroup stop did not persist pids.current"
grep -Fq 'cgroup_pids_peak=7' "$cgroup_metrics" ||
    fail "mock cgroup stop did not persist pids.peak"
grep -Fq 'cgroup_pids_events_max=3' "$cgroup_metrics" ||
    fail "mock cgroup stop did not persist pids.events"

# A forked compiler parent/child can each report the same CoW pages in VmRSS.
# Prove that a verified cgroup records this summed value but does not stop until
# physical cgroup memory reaches the lower early-stop threshold.
printf '%s\n' '1048576' > "$mock_cgroup_dir/memory.current"
printf '%s\n' '2097152' > "$mock_cgroup_dir/memory.peak"
printf '%s\n' '6' > "$mock_cgroup_dir/pids.current"
printf '%s\n' '9' > "$mock_cgroup_dir/pids.peak"
printf '%s\n' 'max 0' > "$mock_cgroup_dir/pids.events"
printf '%s\n' 'max 0' > "$mock_cgroup_dir/pids.events.local"
shared_rss_metrics="$TEST_ROOT/shared-rss-telemetry.metrics"
SEEN_MEMORY_GUARD_IN_SCOPE=1 \
SEEN_MEMORY_GUARD_SCOPE_OWNER=0 \
SEEN_MEMORY_GUARD_KERNEL_SCOPE=0 \
SEEN_MEMORY_GUARD_TEST_HOOKS=1 \
SEEN_MEMORY_GUARD_TEST_CGROUP_DIR="$mock_cgroup_dir" \
SEEN_MEMORY_GUARD_TEST_TREE_RSS_KB=8192 \
    "$GUARD" --rss-limit-kb 4096 --cgroup-stop-kb 3072 \
        --interval-secs 0.05 --kill-only \
        --metrics-file "$shared_rss_metrics" -- sleep 0.15 \
        >"$TEST_ROOT/shared-rss.out" 2>"$TEST_ROOT/shared-rss.err"
status=$?
[ "$status" -eq 0 ] || fail "shared-RSS telemetry-only mock returned $status"
grep -Fq 'state=complete' "$shared_rss_metrics" ||
    fail "shared-RSS mock did not complete below physical cgroup threshold"
grep -Fq 'peak_rss_kb=8192' "$shared_rss_metrics" ||
    fail "shared-RSS mock did not retain summed RSS telemetry"
grep -Fq 'peak_cgroup_kb=2048' "$shared_rss_metrics" ||
    fail "shared-RSS mock did not retain native cgroup peak"
grep -Fq 'summed_rss_role=telemetry_only' "$shared_rss_metrics" ||
    fail "shared-RSS mock mislabeled summed RSS authority"

# pids.max counts threads and transient monitor helpers, not process rows.
# Mock a full 16-task cgroup and prove the builtin monitor completes without
# attempting the former ps|awk pipeline or turning telemetry into a task stop.
printf '%s\n' '16' > "$mock_cgroup_dir/pids.current"
printf '%s\n' '16' > "$mock_cgroup_dir/pids.peak"
pids_overhead_metrics="$TEST_ROOT/pids16-overhead.metrics"
SEEN_MEMORY_GUARD_IN_SCOPE=1 \
SEEN_MEMORY_GUARD_SCOPE_OWNER=0 \
SEEN_MEMORY_GUARD_KERNEL_SCOPE=0 \
SEEN_MEMORY_GUARD_TEST_HOOKS=1 \
SEEN_MEMORY_GUARD_TEST_CGROUP_DIR="$mock_cgroup_dir" \
SEEN_MEMORY_GUARD_TEST_TREE_RSS_KB=512 \
    "$GUARD" --rss-limit-kb 4096 --cgroup-stop-kb 3072 \
        --tasks-max 16 --interval-secs 0.05 --kill-only \
        --metrics-file "$pids_overhead_metrics" -- sleep 0.15 \
        >"$TEST_ROOT/pids16-overhead.out" \
        2>"$TEST_ROOT/pids16-overhead.err"
status=$?
[ "$status" -eq 0 ] || fail "pids16 builtin-monitor mock returned $status"
grep -Fq 'cgroup_pids_current=16' "$pids_overhead_metrics" ||
    fail "pids16 mock did not persist pids.current"
grep -Fq 'cgroup_pids_peak=16' "$pids_overhead_metrics" ||
    fail "pids16 mock did not persist pids.peak"
if grep -Fqi 'fork: retry\|resource temporarily unavailable' \
    "$TEST_ROOT/pids16-overhead.err"; then

    fail "pids16 builtin monitor attempted task-heavy polling"
fi

# A real pids.max rejection is terminal even if the rejected child catches
# EAGAIN. Increment the mocked controller event after monitor initialization
# and prove the owner records one exact 137 without fallback.
printf '%s\n' '8' > "$mock_cgroup_dir/pids.current"
printf '%s\n' 'max 0' > "$mock_cgroup_dir/pids.events"
printf '%s\n' 'max 0' > "$mock_cgroup_dir/pids.events.local"
pids_event_metrics="$TEST_ROOT/pids-event-stop.metrics"
(
    sleep 0.20
    printf '%s\n' 'max 1' > "$mock_cgroup_dir/pids.events"
    printf '%s\n' 'max 1' > "$mock_cgroup_dir/pids.events.local"
) &
pids_event_updater=$!
SEEN_MEMORY_GUARD_IN_SCOPE=1 \
SEEN_MEMORY_GUARD_SCOPE_OWNER=1 \
SEEN_MEMORY_GUARD_KERNEL_SCOPE=0 \
SEEN_MEMORY_GUARD_TEST_HOOKS=1 \
SEEN_MEMORY_GUARD_TEST_CGROUP_DIR="$mock_cgroup_dir" \
SEEN_MEMORY_GUARD_TEST_TREE_RSS_KB=512 \
    "$GUARD" --rss-limit-kb 4096 --cgroup-stop-kb 3072 \
        --tasks-max 16 --interval-secs 0.05 --kill-only \
        --metrics-file "$pids_event_metrics" -- sleep 30 \
        >"$TEST_ROOT/pids-event-stop.out" \
        2>"$TEST_ROOT/pids-event-stop.err"
status=$?
wait "$pids_event_updater" || true
[ "$status" -eq 137 ] || fail "mock pids-controller event returned $status"
grep -Fq 'state=tasks_limit' "$pids_event_metrics" ||
    fail "mock pids-controller event did not record tasks_limit"
[ "$(grep -Fc 'command_status=137' "$pids_event_metrics")" -eq 1 ] ||
    fail "mock pids-controller event lacks one exact terminal status"
grep -Fq 'cgroup_pids_events_max=1' "$pids_event_metrics" ||
    fail "mock pids-controller event was not persisted before tree stop"

hup_child_file="$TEST_ROOT/hup-child.pid"
"${OBSERVER_GUARD_ENV[@]}" "$GUARD" \
        --rss-limit-kb 1048576 --kill-only -- \
        bash -c 'echo $$ > "$1"; sleep 30' bash "$hup_child_file" \
        >"$TEST_ROOT/hup.out" 2>"$TEST_ROOT/hup.err" &
hup_guard_pid=$!
hup_attempt=0
while [ ! -s "$hup_child_file" ] && [ "$hup_attempt" -lt 100 ]; do
    sleep 0.01
    hup_attempt=$((hup_attempt + 1))
done
[ -s "$hup_child_file" ] || {
    kill -KILL "$hup_guard_pid" 2>/dev/null || true
    fail "HUP test child did not start"
}
kill -HUP "$hup_guard_pid"
hup_status=0
wait "$hup_guard_pid" || hup_status=$?
[ "$hup_status" -eq 143 ] || fail "HUP cleanup returned $hup_status"
hup_child_pid=$(cat "$hup_child_file")
if kill -0 "$hup_child_pid" 2>/dev/null; then
    kill -KILL "$hup_child_pid" 2>/dev/null || true
    fail "HUP cleanup left the guarded child alive"
fi

printf '%s\n' '#!/usr/bin/env bash' 'trap "" HUP' 'echo $$ > "$1"' \
    'printf "%s\n" "${SEEN_MEMORY_GUARD_PROCESS_TOKENS:-}" > "$1.tokens"' \
    'exec sleep 30' \
    > "$TEST_ROOT/detached-worker"
printf '%s\n' '#!/usr/bin/env bash' \
    'nohup setsid "$2" "$1" </dev/null >/dev/null 2>&1 &' \
    'worker_job=$!' \
    'for _ in 1 2 3 4 5 6 7 8 9 10; do [ -s "$1" ] && break; sleep 0.01; done' \
    'disown "$worker_job"' \
    > "$TEST_ROOT/detached-launcher"
chmod +x "$TEST_ROOT/detached-worker" "$TEST_ROOT/detached-launcher"
detached_pid_file="$TEST_ROOT/detached-child.pid"
detached_metrics="$TEST_ROOT/detached.metrics"
"${OBSERVER_GUARD_ENV[@]}" "$GUARD" \
        --rss-limit-kb 1048576 --kill-only \
        --metrics-file "$detached_metrics" -- \
        "$TEST_ROOT/detached-launcher" "$detached_pid_file" \
            "$TEST_ROOT/detached-worker" \
        >"$TEST_ROOT/detached.out" 2>"$TEST_ROOT/detached.err"
status=$?
[ "$status" -eq 125 ] || fail "detached process-group cleanup returned $status"
grep -Fq 'state=detached_descendants' "$detached_metrics" ||
    fail "detached cleanup metrics missing"
if [ -s "$detached_pid_file" ]; then
    detached_pid=$(cat "$detached_pid_file")
    detached_attempt=0
    while kill -0 "$detached_pid" 2>/dev/null && [ "$detached_attempt" -lt 100 ]; do
        sleep 0.01
        detached_attempt=$((detached_attempt + 1))
    done
    if kill -0 "$detached_pid" 2>/dev/null; then
        kill -KILL "$detached_pid" 2>/dev/null || true
        fail "detached process-group cleanup left the worker alive"
    fi
fi

echo "memory guard fail-closed checks passed"
