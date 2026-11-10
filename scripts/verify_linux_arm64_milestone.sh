#!/usr/bin/env bash
# Non-publishing native qualification. The input archive is never an ARM proof.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
SELF="$ROOT_DIR/scripts/verify_linux_arm64_milestone.sh"
HELPER="$ROOT_DIR/scripts/linux_arm64_milestone_report.py"
INPUTS="$ROOT_DIR/scripts/ci_linux_arm64_inputs.py"
fail() { echo "native-arm64: $*" >&2; exit 126; }
[ "$(uname -s)" = Linux ] || fail 'real Linux ARM64 execution is required'
case "$(uname -m)" in aarch64|arm64) ;; *) fail 'real Linux ARM64 execution is required; no emulation or x64 fallback' ;; esac

if [ "${SEEN_ARM64_EXECUTOR_CHILD:-0}" = 1 ]; then
    [ "$#" = 0 ] || fail 'unexpected child arguments'
    : "${SEEN_ARM64_EXECUTOR_WORK:?}"
    # safe_rebuild has produced a fresh, scope-bound serializer. Recheck the
    # transported bytes before capped admission can execute the native compiler.
    python3 "$HELPER" verify-inputs --root "$ROOT_DIR" --work "$SEEN_ARM64_EXECUTOR_WORK"
    exec bash "$ROOT_DIR/scripts/run_capped_regression.sh" seen-inf-simd-c \
        --compiler "$SEEN_ARM64_EXECUTOR_WORK/inputs/seen" -- \
        python3 "$HELPER" run --root "$ROOT_DIR" --work "$SEEN_ARM64_EXECUTOR_WORK"
fi

candidate= archive= report=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --candidate) [ "$#" -ge 2 ] && [ -z "$candidate" ] || fail '--candidate requires one directory'; candidate=$2; shift 2 ;;
        --archive) [ "$#" -ge 2 ] && [ -z "$archive" ] || fail '--archive requires one file'; archive=$2; shift 2 ;;
        --report) [ "$#" -ge 2 ] && [ -z "$report" ] || fail '--report requires one new file'; report=$2; shift 2 ;;
        *) fail 'usage: verify_linux_arm64_milestone.sh --candidate DIR --archive ARCHIVE --report NEW_FILE' ;;
    esac
done
[ -n "$candidate" ] && [ -n "$archive" ] && [ -n "$report" ] || fail 'candidate, archive and new report are required'
# CI owns one fresh hard scope around the complete native phase. Reusing that
# scope is permitted only after live kernel read-back; the routing marker is
# not admission. Standalone invocations still own their fresh scope.
if [ "${SEEN_CI_CONTAINMENT_IN_SCOPE:-0}" = 1 ]; then
    [ "${SEEN_CI_PHASE:-}" = native-arm ] || fail 'unexpected inherited CI phase'
    bash "$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only >/dev/null ||
        fail 'inherited CI hard scope failed live kernel read-back'
    [ "${SEEN_ARTIFACT_NAMESPACE_ACTIVE:-0}" != 1 ] &&
        [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != 1 ] ||
        fail 'CI qualification must start before the compiler namespace'
else
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != 1 ] &&
        [ "${SEEN_ARTIFACT_NAMESPACE_ACTIVE:-0}" != 1 ] &&
        [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != 1 ] ||
        fail 'invoke outside an existing compiler scope/namespace'
fi
source "$ROOT_DIR/scripts/artifact_root.sh"
seen_artifact_root_init "$ROOT_DIR"
scope=$(seen_artifact_scope_init linux-arm64-milestone)
work=$(seen_artifact_mktemp_dir "$scope" run)
printf 'Native ARM64 bounded evidence directory: %s\n' "$work" >&2
# Limit every generated file, including setup/suite logs; hitting the limit is
# a real failure, not log truncation followed by a qualification claim.
ulimit -c 0
ulimit -f 65536
python3 "$HELPER" prepare --root "$ROOT_DIR" --work "$work" \
    --candidate "$candidate" --archive "$archive" --report "$report"
status=0
env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_SO -u SEEN_FORK_SERIALIZER_ATTESTATION \
    -u SEEN_RELEASE_CPU_BASELINE -u SEEN_ATTESTED_SEEN_PATH -u SEEN_DATA_PATH \
    -u FIXTURE_ASAN_PRELOAD_LIBRARY -u FIXTURE_ASAN_ADDRESS_LAYOUT \
    SEEN_ARM64_EXECUTOR_CHILD=1 SEEN_ARM64_EXECUTOR_WORK="$work" \
    SEEN_STAGE_BUILDER="$work/inputs/seen" SEEN_STAGE_BUILDER_ONLY=1 \
    SEEN_PACKAGE_CLIENT="$work/inputs/seen-pkg" \
    SEEN_STAGE_BUILDER_PACKAGE_CLIENT="$work/inputs/seen-pkg" \
    SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR" \
    SEEN_PROJECT_ROOT="$ROOT_DIR" SEEN_LOW_MEMORY=1 SEEN_JOBS=1 SEEN_OPT_JOBS=1 \
    SEEN_DISABLE_IR_RECOVERY=1 SEEN_KEEP_TMP=1 SEEN_REQUIRE_ARM_PERF=1 \
    FIXTURE_TARGET_CPU=native FIXTURE_RUN_SECONDS=120 \
    SEEN_TIER_TIMEOUT_SECS="${SEEN_TIER_TIMEOUT_SECS:-2400}" \
    bash "$ROOT_DIR/scripts/safe_rebuild.sh" --regression-script "$SELF" \
    >"$work/setup.log" 2>&1 || status=$?
if [ "$status" -ne 0 ]; then
    printf 'Native ARM64 qualification failed (status %s); no report published. Logs: %s\n' "$status" "$work" >&2
    if ! python3 - "$ROOT_DIR" "$work/setup.log" <<'PY'
import sys
from pathlib import Path
root, path = map(Path, sys.argv[1:])
sys.path.insert(0, str(root / "scripts"))
from collect_verification_logs import read_tail
_, content, _ = read_tail(root, path, 32768)
sys.stderr.buffer.write(content)
PY
    then
        printf 'Native ARM64 setup diagnostic tail unavailable; original failure status preserved.\n' >&2
    fi
    exit "$status"
fi
# Publication follows the complete guarded route, not merely the fixture child.
python3 "$HELPER" publish --root "$ROOT_DIR" --work "$work"
