#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-p0-accel-gate-c
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
TMP_DIR="$(mktemp -d "${SEEN_ARTIFACT_ROOT:?}/p0-accel-c.XXXXXX")"
COLLECTOR="$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/runtime_child_evidence.py"
# Preserve successful and failed nested reports; scope re-entry below retains
# the real inherited capped cgroup and the compiler's serializer attestation.
export SEEN_KEEP_TMP=1
ulimit -c 0
ulimit -S -f 65536
printf '[]\n' > "$TMP_DIR/checks.json"
record() {
    local name=$1 status=$2 detail=$3
    shift 3
    python3 - "$TMP_DIR/checks.json" "$name" "$status" "$detail" "$@" <<'PY'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
rows = json.loads(path.read_text())
rows.append({'name': sys.argv[2], 'exit_status': int(sys.argv[3]),
             'execution': 'native-cpu', 'classification': 'compile-only',
             'required': True, 'logs': sys.argv[5:],
             'details': {'evidence_boundary': sys.argv[4], 'production_certified': False}})
path.write_text(json.dumps(rows, indent=2) + '\n')
PY
}
finish() {
    local status=$? report_status=0
    trap - EXIT
    if [ "$status" -ne 0 ]; then
        printf 'Harness exit status: %s\n' "$status" > "$TMP_DIR/harness-exit.log"
        record harness-exit "$status" \
            'Aggregate infrastructure failure; preceding child results remain retained' harness-exit.log || :
    fi
    python3 "$ROOT_DIR/scripts/accelerator_gate_evidence.py" --phase c \
        --compiler "$COMPILER" --root "$ROOT_DIR" --artifacts "$TMP_DIR" \
        --checks "$TMP_DIR/checks.json" --output "$TMP_DIR/accelerator-c-evidence.json" || report_status=$?
    printf 'Runtime lifecycle/trace evidence retained: %s\n' "$TMP_DIR" >&2
    if [ "$status" -ne 0 ]; then exit "$status"; fi
    exit "$report_status"
}
trap finish EXIT
children=(seen_gpu_runtime_a seen_gpu_runtime_a_vulkan seen_gpu_runtime_b seen_gpu_runtime_c)
children+=(
    seen_gpu_runtime_d seen_gpu_runtime_e seen_gpu_runtime_f seen_gpu_runtime_g seen_gpu_runtime_h
    seen_gpu_vulkan_mapping seen_gpu_vulkan_transfer seen_gpu_vulkan_staging
    seen_cuda_event_query seen_gpu_cuda_staging seen_gpu_cuda_peer
)
for child in "${children[@]}"; do
    boundary=software-reference
    hardware_opt_in=unset
    case "$child" in
        seen_gpu_runtime_a) prefix=gpu-runtime-a; boundary=cpu ;;
        seen_gpu_runtime_d) prefix=gpu-vulkan-stream ;;
        seen_gpu_runtime_e) prefix=gpu-vulkan-pipeline ;;
        seen_cuda_event_query)
            prefix=cuda-event-query; boundary=cuda-hardware
            hardware_opt_in="${SEEN_CUDA_QUERY_HARDWARE-unset}" ;;
        seen_gpu_cuda_staging)
            prefix=gpu-cuda-staging; boundary=cuda-hardware
            hardware_opt_in="${SEEN_CUDA_STAGING_HARDWARE-unset}" ;;
        seen_gpu_cuda_peer)
            prefix=gpu-cuda-peer; boundary=cuda-hardware
            hardware_opt_in="${SEEN_CUDA_PEER_HARDWARE-unset}" ;;
        *) prefix="${child#seen_}"; prefix="${prefix//_/-}" ;;
    esac
    if [ "$child" = seen_gpu_runtime_e ]; then
        hardware_opt_in="${SEEN_VULKAN_PIPELINE_HARDWARE-unset}"
    fi
    snapshot_status=0
    timeout --foreground --kill-after=5s 30s python3 "$COLLECTOR" --artifacts "$SEEN_ARTIFACT_ROOT" --aggregate "$TMP_DIR" \
        --prefix "$prefix" --snapshot "$TMP_DIR/$child.before.json" --before \
        > "$TMP_DIR/$child.snapshot.log" 2>&1 || snapshot_status=$?
    if [ "$snapshot_status" -ne 0 ]; then
        record "$child-artifact-snapshot" "$snapshot_status" \
            'Failed artifact discovery is retained without skipping the required child' "$child.snapshot.log"
    fi
    status=0
    timeout --foreground --kill-after=5s 3600s env SEEN_CAPPED_REGRESSION_ACTIVE=0 \
        COMPILER="$COMPILER" SEEN_KEEP_TMP=1 bash "$ROOT_DIR/tests/misc_root_tests/$child.sh" \
        > "$TMP_DIR/$child.log" 2>&1 || status=$?
    printf '%s\n' "$status" > "$TMP_DIR/$child.status"
    record "$child" "$status" \
        'Full existing child gate executed serially, with every native/sanitizer/fault/coverage contract it requires; this row denotes the CPU harness/compiler, not proof of GPU execution. Actual completed runtime profiles and nested reports are recorded separately.' \
        "$child.log" "$child.status"
    # Observe even failed children: successful early profiles do not hide later
    # ASan, lifecycle, native-driver, or trace failures in the required row.
    collector_status=0
    timeout --foreground --kill-after=5s 120s python3 "$COLLECTOR" --artifacts "$SEEN_ARTIFACT_ROOT" --aggregate "$TMP_DIR" \
        --prefix "$prefix" --snapshot "$TMP_DIR/$child.before.json" \
        --child "$child" --status "$status" --boundary "$boundary" \
        --hardware-opt-in "$hardware_opt_in" > "$TMP_DIR/$child.observation.log" 2>&1 || collector_status=$?
    printf '%s\n' "$collector_status" > "$TMP_DIR/$child.observation.status"
    record "$child-observation" "$collector_status" \
        'Parse this run only: retained child statuses/raw logs and genuine nested reports, with no PASS-text assertion or source pins; absent hardware observations remain unsupported/unexecuted, not synthetic successes.' \
        "$child.observation.log" "$child.observation.status"
done
