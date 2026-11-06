#!/usr/bin/env bash
# No-build rejection checks; resource policy is enforced by the real runners.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
RUNNER="$ROOT_DIR/scripts/run_attested_seen.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
fail() { echo "FAIL: capped regression rejection: $*" >&2; exit 1; }
bash -n "$ENTRY" "$RUNNER" "$ASAN_RUNNER" "$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = 1 ]; then
    source "$ARTIFACT_HELPER"
    source "$ROOT_DIR/scripts/serial_auxiliary_env.sh"
    seen_artifact_root_init "$ROOT_DIR"
    seen_serial_auxiliary_verify "$ROOT_DIR" "$SEEN_ARTIFACT_ROOT"
fi
if env -u SEEN_CAPPED_REGRESSION_ACTIVE -u SEEN_CAPPED_REGRESSION_SCOPE \
    -u SEEN_CAPPED_REGRESSION_COMPILER -u SEEN_CAPPED_REGRESSION_TOOLCHAIN \
    -u SEEN_ATTESTED_COMPILER_RUNNER \
    bash "$ASAN_RUNNER" --target-root /bin --compile-log /dev/null -- /bin/true \
    >/dev/null 2>&1; then
    fail 'sanitizer execution outside an attested regression was accepted'
else
    status=$?
    [ "$status" -eq 126 ] || fail "outside-scope rejection returned $status instead of126"
fi
source "$ARTIFACT_HELPER"
seen_artifact_root_init "$ROOT_DIR"
probe_scope=$(seen_artifact_scope_init capped-asan-runner-probe)
probe_root=$(seen_artifact_mktemp_dir "$probe_scope" run)
cleanup() {
    local status=$?
    trap - EXIT
    case "$probe_root" in
        "$probe_scope"/run.*)
            if [ -d "$probe_root" ] && [ ! -L "$probe_root" ] &&
                [ "$(dirname -- "$probe_root")" = "$probe_scope" ]; then
                rm -rf -- "$probe_root" || status=1
            else
                status=1
            fi ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT
mkdir -p -- "$probe_root/target" "$probe_root/logs"
cp -- /bin/true "$probe_root/target/not-sanitized"
chmod 0755 "$probe_root/target/not-sanitized"
for sanitizer in address thread; do
    printf '  Sanitizer: %s\n' "$sanitizer" > "$probe_root/logs/$sanitizer.compile.log"
    if env SEEN_CAPPED_REGRESSION_ACTIVE=1 \
        SEEN_CAPPED_REGRESSION_SCOPE=forged-sanitizer-probe \
        SEEN_CAPPED_REGRESSION_COMPILER=/bin/true \
        SEEN_CAPPED_REGRESSION_TOOLCHAIN="$probe_root/target" \
        SEEN_ATTESTED_COMPILER_RUNNER=/bin/true \
        SEEN_MEMORY_GUARD_IN_SCOPE=1 SEEN_HARD_MEMORY_SCOPE_ACTIVE=1 \
        SEEN_MEMORY_GUARD_SCOPE_UNIT=forged.scope SEEN_MAIN_VMEM_KB=1048576 \
        bash "$ASAN_RUNNER" --sanitizer "$sanitizer" \
        --target-root "$probe_root/target" \
        --compile-log "$probe_root/logs/$sanitizer.compile.log" -- \
        "$probe_root/target/not-sanitized" > "$probe_root/probe.out" 2> "$probe_root/probe.err"; then
        fail 'forged sanitizer markers admitted an uninstrumented executable'
    else
        status=$?
        [ "$status" -eq 126 ] || fail "uninstrumented rejection returned $status instead of126"
    fi
    # Distinguish artifact rejection from a later, unrelated forged-scope error.
    grep -Fq "no $sanitizer-sanitizer runtime symbol" "$probe_root/probe.err" ||
        fail 'uninstrumented artifact reached the wrong rejection boundary'
done
echo 'PASS: out-of-scope and forged sanitizer execution are rejected'
