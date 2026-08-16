#!/usr/bin/env bash
# Static, no-build regression checks for performance-entry containment.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
PERF_GATE="$ROOT_DIR/scripts/perf_gate.sh"
PRODUCTION_BENCH="$ROOT_DIR/scripts/run_production_benchmarks.sh"
PERF_BISECT="$ROOT_DIR/scripts/perf_bisect.sh"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

line_of() {
    local file=$1
    local pattern=$2
    local line
    line=$(grep -nF -- "$pattern" "$file" | tail -1 | cut -d: -f1)
    case "$line" in
        ''|*[!0-9]*) fail "could not locate: $pattern" ;;
    esac
    printf '%s\n' "$line"
}

bash -n "$PERF_GATE" "$PRODUCTION_BENCH" "$PERF_BISECT" ||
    fail "performance entry-point syntax"
grep -Fq 'ORIGINAL_ARGS=("$@")' "$PERF_GATE" ||
    fail "original argv is not captured before parsing"
[ "$(grep -Fc '"$SELF_PATH" "${ORIGINAL_ARGS[@]}"' "$PERF_GATE")" -eq 2 ] ||
    fail "artifact and hard-scope re-entry do not both preserve argv"

grep -Fq 'run_with_project_artifacts.sh' "$PERF_GATE" ||
    fail "project-artifact wrapper is missing"
grep -Fq 'run_in_hard_memory_scope.sh' "$PERF_GATE" ||
    fail "hard-memory-scope wrapper is missing"
grep -Fq '[ "$namespace_tmp_identity" != "$artifact_root_identity" ]' "$PERF_GATE" ||
    fail "forged artifact markers can bypass namespace inode read-back"
grep -Fq -- '--verify-only --' "$PERF_GATE" ||
    fail "forged hard-scope markers can bypass cgroup read-back"
grep -Fq 'SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=1' "$PERF_GATE" ||
    fail "nested performance commands do not require the inherited kernel scope"
grep -Fq 'SEEN_MEMORY_GUARD_SCOPE_OWNER=0' "$PERF_GATE" ||
    fail "nested performance guards can claim aggregate-scope ownership"

readback_line=$(line_of "$PERF_GATE" '    --verify-only --')
output_init_line=$(line_of "$PERF_GATE" 'ensure_perf_directory "$SEEN_PERF_OUTPUT_ROOT"')
trace_init_line=$(line_of "$PERF_GATE" 'seen_build_trace_init "perf_gate"')
[ "$readback_line" -lt "$output_init_line" ] ||
    fail "performance outputs can be initialized before cgroup read-back"
[ "$output_init_line" -lt "$trace_init_line" ] ||
    fail "build tracing can initialize before the validated output directories"

grep -Fq 'case "$candidate" in' "$PERF_GATE" ||
    fail "performance output lacks an explicit repository boundary check"
grep -Fq '"$ROOT_DIR"/*) relative_path=${candidate#"$ROOT_DIR"/}' "$PERF_GATE" ||
    fail "performance output boundary is not rooted in the checkout"
grep -Fq 'git -C "$ROOT_DIR" check-ignore -q -- "$relative_path"' "$PERF_GATE" ||
    fail "performance output is not required to be Git-ignored"
grep -Fq 'seen_artifact_assert_no_symlink_components "$ROOT_DIR" "$relative_path"' \
    "$PERF_GATE" || fail "performance output can traverse a symbolic link"
if grep -Fq 'target/seen-build/perf-' "$PERF_GATE" ||
    grep -Fq 'target/seen-build/traces' "$PERF_GATE"; then

    fail "legacy performance output paths remain outside the validated namespace"
fi

grep -Fq 'export SEEN_JOBS=1' "$PERF_GATE" ||
    fail "performance gate does not force one compiler worker"
grep -Fq 'export SEEN_OPT_JOBS=1' "$PERF_GATE" ||
    fail "performance gate does not force one optimizer worker"
[ "$(grep -Fc '4194304' "$PERF_GATE")" -ge 2 ] ||
    fail "performance gate does not derive and enforce the 4 GiB hard ceiling"
grep -Fq '[ "$opt_kb" -gt 2097152 ]' "$PERF_GATE" ||
    fail "performance gate does not enforce the 2 GiB optimizer ceiling"
grep -Fq 'advertised-jobs) PERF_COMPILER_FLAGS=(--jobs 1 --opt-jobs 1)' "$PERF_GATE" ||
    fail "advertised worker flags are not passed explicitly"
grep -Fq 'serializer-required) PERF_COMPILER_FLAGS=()' "$PERF_GATE" ||
    fail "unflagged compilers are not routed through authoritative serialization"
grep -Fq 'verify_fork_serializer.sh' "$PERF_GATE" ||
    fail "performance gate does not verify serializer attestation"
grep -Fq 'rebuild_builder_applicability.sh' "$PERF_GATE" ||
    fail "performance gate does not reject inapplicable compilers"
grep -Fq 'prepare_bounded_toolchain.sh' "$PERF_GATE" ||
    fail "performance gate does not cap optimizer/link helpers"
grep -Fq 'SEEN_MEMORY_LIMIT_BYTES="$memory_limit_bytes"' "$PERF_GATE" ||
    fail "build-suite re-entry does not preserve the verified allocation cap"

for direct_entry in "$PRODUCTION_BENCH" "$PERF_BISECT"; do
    grep -Fq 'ORIGINAL_ARGS=("$@")' "$direct_entry" ||
        fail "direct performance entry does not capture argv: $direct_entry"
    [ "$(grep -Fc '"$SELF_PATH" "${ORIGINAL_ARGS[@]}"' "$direct_entry")" -eq 2 ] ||
        fail "direct performance re-entry does not preserve argv: $direct_entry"
    grep -Fq 'run_with_project_artifacts.sh' "$direct_entry" ||
        fail "direct performance entry lacks artifact wrapper: $direct_entry"
    grep -Fq 'run_in_hard_memory_scope.sh' "$direct_entry" ||
        fail "direct performance entry lacks hard-scope wrapper: $direct_entry"
    grep -Fq '[ "$namespace_tmp_identity" != "$artifact_root_identity" ]' \
        "$direct_entry" || fail "forged artifact marker bypass: $direct_entry"
    grep -Fq -- '--verify-only --' "$direct_entry" ||
        fail "forged hard-scope marker bypass: $direct_entry"
    grep -Fq 'export SEEN_LOW_MEMORY=1 SEEN_JOBS=1 SEEN_OPT_JOBS=1' \
        "$direct_entry" || fail "direct performance entry is not serial: $direct_entry"
    [ "$(grep -Fc '4194304' "$direct_entry")" -ge 2 ] ||
        fail "direct performance entry lacks 4 GiB ceiling: $direct_entry"
    grep -Fq '[ "$opt_kb" -gt 2097152 ]' "$direct_entry" ||
        fail "direct performance entry lacks 2 GiB optimizer ceiling: $direct_entry"
done

production_readback=$(line_of "$PRODUCTION_BENCH" '    --verify-only --')
production_output=$(line_of "$PRODUCTION_BENCH" \
    'mkdir -p -- "$SEEN_PRODUCTION_BENCH_OUTPUT_ROOT"')
production_compile=$(line_of "$PRODUCTION_BENCH" \
    'COMPILE_OUTPUT=$(env -u SEEN_FORK_SERIALIZER_ROOT_PID')
[ "$production_readback" -lt "$production_output" ] ||
    fail "production benchmark results initialize before hard-scope read-back"
[ "$production_output" -lt "$production_compile" ] ||
    fail "production benchmark can compile before project output initialization"
grep -Fq 'git -C "$REPO_ROOT" check-ignore -q -- "$relative_path"' \
    "$PRODUCTION_BENCH" || fail "production results are not required to be ignored"
grep -Fq 'OUTPUT_BIN="$BENCH_BIN_ROOT/bench_${num}"' "$PRODUCTION_BENCH" ||
    fail "production benchmark binary is not project-local"
grep -Fq 'advertised-jobs) COMPILER_WORKER_FLAGS=(--jobs 1 --opt-jobs 1)' \
    "$PRODUCTION_BENCH" || fail "production compiler does not receive advertised worker flags"
grep -Fq 'verify_fork_serializer.sh' "$PRODUCTION_BENCH" ||
    fail "production benchmark does not verify serializer attestation"
grep -Fq 'rebuild_builder_applicability.sh' "$PRODUCTION_BENCH" ||
    fail "production benchmark does not check serializer applicability"
grep -Fq 'prepare_bounded_toolchain.sh' "$PRODUCTION_BENCH" ||
    fail "production benchmark does not cap optimizer/link helpers"
if grep -Fq '2>&1 || true)' "$PRODUCTION_BENCH"; then
    fail "production benchmark erases compiler status"
fi
grep -Fq 'abort_on_resource_failure "$COMPILE_STATUS"' "$PRODUCTION_BENCH" ||
    fail "production benchmark can continue after a compiler resource stop"
grep -Fq '[ -L "$OUTPUT_BIN" ]' "$PRODUCTION_BENCH" ||
    fail "production benchmark accepts a symlink binary target"
if grep -Fq 'OUTPUT_BIN="/tmp/' "$PRODUCTION_BENCH" ||
    grep -Fq 'RESULTS_DIR="target/' "$PRODUCTION_BENCH"; then

    fail "production benchmark retains a host/global artifact path"
fi

bisect_readback=$(line_of "$PERF_BISECT" '--verify-only --')
bisect_disable=$(line_of "$PERF_BISECT" \
    'historical performance builds are disabled until a current, scope-owned bisect driver')
[ "$bisect_readback" -lt "$bisect_disable" ] ||
    fail "performance bisect does not verify containment before failing closed"
grep -Fq 'No revision was checked out and no compiler or benchmark was executed.' \
    "$PERF_BISECT" || fail "performance bisect lacks a clear fail-closed diagnostic"
if grep -Eq 'git (bisect|checkout)|safe_rebuild\.sh|["$]compiler[" ]+compile' "$PERF_BISECT" ||
    grep -Fq 'stage1_frozen' "$PERF_BISECT" ||
    grep -Fq 'mktemp /tmp/' "$PERF_BISECT" ||
    grep -Fq 'OUTPUT_BIN="/tmp/' "$PERF_BISECT"; then

    fail "performance bisect retains historical build execution or host artifacts"
fi

echo "PASS: performance entry hard-scope and artifact containment are statically enforced"
