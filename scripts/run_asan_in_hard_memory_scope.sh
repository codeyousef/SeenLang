#!/usr/bin/env bash
# Request finite shadow virtual address space for a generated Linux sanitizer
# executable. Native ARM address/default admission additionally requires fresh
# pinned LLVM20 controls; architecture or a caller's receipt cannot authorize it.
# Physical memory, swap, task, worker and timeout containment remain enforced.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
ARTIFACT_ROOT_HELPER="$SCRIPT_DIR/artifact_root.sh"
HARD_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
CAPPED_ENTRY="$SCRIPT_DIR/run_capped_regression.sh"
ASAN_VMEM_LIMIT_KB=68719476736
TSAN_VMEM_LIMIT_KB=137438953472
SANITIZER=address
SANITIZER_VMEM_LIMIT_KB=$ASAN_VMEM_LIMIT_KB
ADDRESS_LAYOUT=default
MAX_COMPILE_LOG_BYTES=16777216

fail() {
    echo "RESOURCE STOP: sanitizer hard-scope runner: $*" >&2
    exit 126
}

is_positive_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

TARGET_ROOT=""
COMPILE_LOG=""
FIXTURE_PRELOAD=""
FIXTURE_PRELOAD_SHA256=""
DRIVER_LEAK_PROFILE=""
DRIVER_LEAK_CONTROL=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --sanitizer)
            [ "$#" -ge 2 ] || fail "--sanitizer requires a value"
            SANITIZER=$2
            shift 2
            ;;
        --address-layout)
            [ "$#" -ge 2 ] && [ "$ADDRESS_LAYOUT" = default ] || fail "--address-layout requires one value"
            ADDRESS_LAYOUT=$2
            shift 2
            ;;
        --target-root)
            [ "$#" -ge 2 ] || fail "--target-root requires a value"
            TARGET_ROOT=$2
            shift 2
            ;;
        --compile-log)
            [ "$#" -ge 2 ] || fail "--compile-log requires a value"
            COMPILE_LOG=$2
            shift 2
            ;;
        --fixture-preload)
            [ "$#" -ge 2 ] && [ -z "$FIXTURE_PRELOAD" ] || fail "--fixture-preload requires one path"
            FIXTURE_PRELOAD=$2
            shift 2
            ;;
        --fixture-preload-sha256)
            [ "$#" -ge 2 ] && [ -z "$FIXTURE_PRELOAD_SHA256" ] || fail "--fixture-preload-sha256 requires one hash"
            FIXTURE_PRELOAD_SHA256=$2
            shift 2
            ;;
        --driver-leak-profile)
            [ "$#" -ge 2 ] && [ -z "$DRIVER_LEAK_PROFILE" ] || fail "--driver-leak-profile requires one value"
            DRIVER_LEAK_PROFILE=$2
            shift 2
            ;;
        --driver-leak-control)
            [ "$#" -ge 2 ] && [ -z "$DRIVER_LEAK_CONTROL" ] || fail "--driver-leak-control requires one path"
            DRIVER_LEAK_CONTROL=$2
            shift 2
            ;;
        --)
            shift
            break
            ;;
        *) fail "unknown option: $1" ;;
    esac
done
case "$SANITIZER" in
    address) runtime_symbol=__asan_init; sanitizer_label=address-sanitizer; shadow_label=ASan ;;
    thread)
        runtime_symbol=__tsan_init; sanitizer_label=thread-sanitizer; shadow_label=TSan
        SANITIZER_VMEM_LIMIT_KB=$TSAN_VMEM_LIMIT_KB
        ;;
    *) fail "--sanitizer must be address or thread" ;;
esac
case "$ADDRESS_LAYOUT" in
    default) address_layout_options=protect_shadow_gap=1 ;;
    cuda)
        [ "$SANITIZER" = address ] || fail "CUDA address layout requires AddressSanitizer"
        # CUDA uses addresses in ASan's default inaccessible shadow gap. LLVM
        # maps additional shadow for this application range in this mode:
        # compiler-rt/lib/asan/asan_shadow_setup.cpp, ProtectGap(). It does not
        # disable poisoning, redzones, quarantine, error exits or leak checks.
        # NVIDIA/nccl makefiles/common.mk documents the CUDA startup requirement.
        address_layout_options=protect_shadow_gap=0
        ;;
    *) fail "--address-layout must be default or cuda" ;;
esac
case "$DRIVER_LEAK_PROFILE" in
    '') [ -z "$DRIVER_LEAK_CONTROL" ] || fail "--driver-leak-control requires --driver-leak-profile" ;;
    cuda|vulkan)
        [ "$SANITIZER" = address ] || fail "GPU driver leak attribution requires AddressSanitizer"
        [ -n "$DRIVER_LEAK_CONTROL" ] || fail "--driver-leak-profile requires an SDK-only --driver-leak-control"
        ;;
    *) fail "--driver-leak-profile must be cuda or vulkan" ;;
esac
[ -n "$TARGET_ROOT" ] && [ -n "$COMPILE_LOG" ] && [ "$#" -ge 1 ] ||
    fail "usage: $0 --target-root DIR --compile-log FILE -- /absolute/generated/executable [args...]"

[ "$(uname -s)" = "Linux" ] || fail "finite sanitizer admission requires Linux"
NATIVE_ARM_ASAN=0
case "$(uname -m)" in
    x86_64) ;;
    aarch64|arm64)
        [ "$SANITIZER" = address ] && [ "$ADDRESS_LAYOUT" = default ] ||
            fail "native ARM admits only controlled default AddressSanitizer, not TSan or CUDA"
        [ -z "$FIXTURE_PRELOAD" ] && [ -z "$FIXTURE_PRELOAD_SHA256" ] ||
            fail "native ARM sanitizer fixture preloads have no reviewed admission"
        [ -z "$DRIVER_LEAK_PROFILE" ] || fail "native ARM has no reviewed GPU driver leak attribution"
        NATIVE_ARM_ASAN=1
        ;;
    *) fail "no finite sanitizer admission exists for this Linux architecture" ;;
esac

REGRESSION_MODE=""
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" = "1" ] &&
    [ -n "${SEEN_CAPPED_REGRESSION_SCOPE:-}" ] &&
    [ -n "${SEEN_CAPPED_REGRESSION_COMPILER:-}" ] &&
    [ -n "${SEEN_CAPPED_REGRESSION_TOOLCHAIN:-}" ] &&
    [ -n "${SEEN_ATTESTED_COMPILER_RUNNER:-}" ]; then

    REGRESSION_MODE=compiler
elif [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" = "1" ] &&
    [ -n "${SEEN_CAPPED_PLATFORM_REGRESSION_SCOPE:-}" ] &&
    [ -n "${SEEN_CAPPED_PLATFORM_REGRESSION_TOOLCHAIN:-}" ]; then

    REGRESSION_MODE=platform
else
    fail "active capped compiler or platform regression bindings are missing"
fi
[ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] &&
    [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" = "1" ] &&
    [ -n "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" ] ||
    fail "verified aggregate hard-scope bindings are missing"

[ -f "$ARTIFACT_ROOT_HELPER" ] && [ ! -L "$ARTIFACT_ROOT_HELPER" ] ||
    fail "artifact-root helper is missing or unsafe"
[ -f "$HARD_SCOPE_WRAPPER" ] && [ ! -L "$HARD_SCOPE_WRAPPER" ] ||
    fail "hard-scope verifier is missing or unsafe"
[ -f "$CAPPED_ENTRY" ] && [ ! -L "$CAPPED_ENTRY" ] ||
    fail "capped-regression verifier is missing or unsafe"
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_HELPER"
seen_artifact_root_init "$REPO_ROOT" || fail "artifact-root validation failed"

requested_binary=$1
shift
case "$TARGET_ROOT" in
    /*) ;;
    *) fail "the regression target root must be absolute" ;;
esac
[ -d "$TARGET_ROOT" ] && [ ! -L "$TARGET_ROOT" ] ||
    fail "the regression target root is missing or unsafe"
canonical_target_root=$(cd -P -- "$TARGET_ROOT" 2>/dev/null && pwd -P) ||
    fail "the regression target root is not resolvable"
[ "$canonical_target_root" = "$TARGET_ROOT" ] ||
    fail "the regression target root is not canonical"
case "$canonical_target_root" in
    "$SEEN_ARTIFACT_ROOT"/*) ;;
    *) fail "the regression target root escaped the validated artifact root" ;;
esac

# Inherited preload state remains forbidden. A fixture may explicitly bind a
# separately compiled SDK fault library in its exact target directory. This
# does not relax sanitizer options, scope admission or resource limits.
fixture_preload_env=()
if [ -n "$FIXTURE_PRELOAD" ] || [ -n "$FIXTURE_PRELOAD_SHA256" ]; then
    [[ "$FIXTURE_PRELOAD_SHA256" =~ ^[a-f0-9]{64}$ ]] || fail "fixture library requires a SHA-256 identity"
    [ "${FIXTURE_PRELOAD%/*}" = "$canonical_target_root" ] &&
        [ -f "$FIXTURE_PRELOAD" ] && [ ! -L "$FIXTURE_PRELOAD" ] ||
        fail "fixture library escaped the exact target root or is unsafe"
    case "$FIXTURE_PRELOAD" in *[[:space:]:]*) fail "fixture library path cannot contain preload separators" ;; esac
    fixture_bytes=$(stat -c %s -- "$FIXTURE_PRELOAD" 2>/dev/null || true)
    is_positive_integer "$fixture_bytes" && [ "$fixture_bytes" -le 16777216 ] ||
        fail "fixture library exceeds its bounded size"
    [ "$(sha256sum -- "$FIXTURE_PRELOAD" | cut -d ' ' -f1)" = "$FIXTURE_PRELOAD_SHA256" ] ||
        fail "fixture library SHA-256 mismatch"
    fixture_elf=$(timeout 10 readelf -hW -- "$FIXTURE_PRELOAD" 2>/dev/null) || fail "fixture library is not ELF"
    grep -Eq 'Type:[[:space:]]+DYN \(Shared object file\)' <<<"$fixture_elf" ||
        fail "fixture library must be an ELF shared object"
    fixture_preload_env=("LD_PRELOAD=$FIXTURE_PRELOAD")
fi

case "$requested_binary" in
    /*) ;;
    *) fail "the generated executable path must be absolute" ;;
esac
[ -f "$requested_binary" ] && [ -x "$requested_binary" ] &&
    [ ! -L "$requested_binary" ] ||
    fail "the generated executable must be a regular non-symlink executable"
binary_parent=$(dirname -- "$requested_binary")
binary_name=$(basename -- "$requested_binary")
canonical_parent=$(cd -P -- "$binary_parent" 2>/dev/null && pwd -P) ||
    fail "the generated executable parent is not resolvable"
canonical_binary="$canonical_parent/$binary_name"
[ "$canonical_binary" = "$requested_binary" ] ||
    fail "the generated executable path is not canonical"
[ "$canonical_parent" = "$canonical_target_root" ] ||
    fail "the generated executable is outside the caller's exact regression target root"

case "$COMPILE_LOG" in
    /*) ;;
    *) fail "the address-sanitizer compile log must be absolute" ;;
esac
[ -f "$COMPILE_LOG" ] && [ ! -L "$COMPILE_LOG" ] ||
    fail "the address-sanitizer compile log is missing or unsafe"
compile_log_parent=$(dirname -- "$COMPILE_LOG")
compile_log_name=$(basename -- "$COMPILE_LOG")
canonical_compile_log_parent=$(cd -P -- "$compile_log_parent" 2>/dev/null && pwd -P) ||
    fail "the address-sanitizer compile-log parent is not resolvable"
canonical_compile_log="$canonical_compile_log_parent/$compile_log_name"
[ "$canonical_compile_log" = "$COMPILE_LOG" ] ||
    fail "the address-sanitizer compile log is not canonical"
case "$canonical_compile_log" in
    "$SEEN_ARTIFACT_ROOT"/*) ;;
    *) fail "the address-sanitizer compile log escaped the validated artifact root" ;;
esac
compile_log_bytes=$(stat -c %s -- "$canonical_compile_log" 2>/dev/null || true)
is_positive_integer "$compile_log_bytes" &&
    [ "$compile_log_bytes" -le "$MAX_COMPILE_LOG_BYTES" ] ||
    fail "the address-sanitizer compile log is empty or exceeds the bounded size"
compile_evidence="  Sanitizer: $SANITIZER"
if ! grep -Fq "$compile_evidence" "$canonical_compile_log" &&
    ! grep -Eq "^\+ ([^[:space:]]*/)?clang([[:space:]]|$).*[[:space:]]-fsanitize=$SANITIZER([[:space:]]|$)" \
        "$canonical_compile_log"; then
    fail "the compiler log does not attest $sanitizer_label instrumentation"
fi
has_sanitizer_runtime() {
    timeout 10 readelf -Ws -- "$1" 2>/dev/null |
        awk -v symbol="$runtime_symbol" '{ split($NF, name, "@"); if (name[1] == symbol) found = 1 }
            END { exit(found ? 0 : 1) }'
}
has_sanitizer_runtime "$canonical_binary" ||
    fail "the generated executable has no $sanitizer_label runtime symbol"
if [ -n "$DRIVER_LEAK_PROFILE" ]; then
    # The SDK-only control is the leak baseline; it must be ASan-built in the
    # same exact target root, never an arbitrary or inherited executable.
    [ "${DRIVER_LEAK_CONTROL%/*}" = "$canonical_target_root" ] &&
        [ -f "$DRIVER_LEAK_CONTROL" ] && [ -x "$DRIVER_LEAK_CONTROL" ] && [ ! -L "$DRIVER_LEAK_CONTROL" ] ||
        fail "driver leak control escaped the exact target root or is unsafe"
    [ "$DRIVER_LEAK_CONTROL" != "$canonical_binary" ] || fail "driver leak control cannot be the fixture"
    has_sanitizer_runtime "$DRIVER_LEAK_CONTROL" || fail "driver leak control has no $sanitizer_label runtime symbol"
fi

is_positive_integer "${SEEN_MAIN_VMEM_KB:-}" ||
    fail "the inherited main virtual-memory cap is missing"
current_soft_vmem_kb=$(ulimit -S -v 2>/dev/null || true)
is_positive_integer "$current_soft_vmem_kb" ||
    fail "the inherited soft virtual-memory limit is not finite"
[ "$current_soft_vmem_kb" -le "$SEEN_MAIN_VMEM_KB" ] ||
    fail "the inherited soft virtual-memory limit exceeds the capped-regression budget"

# Revalidate the exact compiler, scope, bounded-toolchain, and serializer
# bindings rather than trusting their inherited marker strings.
if [ "$REGRESSION_MODE" = compiler ]; then
    bash "$CAPPED_ENTRY" --verify-active "$SEEN_CAPPED_REGRESSION_SCOPE" \
        --compiler "$SEEN_CAPPED_REGRESSION_COMPILER" >/dev/null ||
        fail "active capped-regression binding read-back failed"
else
    bash "$CAPPED_ENTRY" --verify-platform-active \
        "$SEEN_CAPPED_PLATFORM_REGRESSION_SCOPE" >/dev/null ||
        fail "active capped-platform binding read-back failed"
fi
# Re-read the cgroup and all serial/low-memory bindings immediately before the
# narrow virtual-address relaxation. This child cannot weaken the parent scope.
bash "$HARD_SCOPE_WRAPPER" \
    --label "$shadow_label generated-executable read-back" --verify-only -- \
    >/dev/null || fail "aggregate hard-memory scope read-back failed"

hard_vmem_kb=$(ulimit -H -v 2>/dev/null || true)
case "$hard_vmem_kb" in
    unlimited) ;;
    *)
        is_positive_integer "$hard_vmem_kb" ||
            fail "the hard virtual-memory limit could not be read"
        [ "$hard_vmem_kb" -ge "$SANITIZER_VMEM_LIMIT_KB" ] ||
            fail "the hard virtual-memory limit cannot admit the finite $shadow_label allowance"
        ;;
esac
if [ "$NATIVE_ARM_ASAN" = 1 ]; then
    arm_admission="$SCRIPT_DIR/linux_arm64_asan_admission.py"
    arm_control="$SCRIPT_DIR/linux_arm64_asan_control.c"
    [ -f "$arm_admission" ] && [ ! -L "$arm_admission" ] &&
        [ -f "$arm_control" ] && [ ! -L "$arm_control" ] ||
        fail "native ARM sanitizer admission owner or control source is missing or unsafe"
    # No recursive runner and no cached authorization: this owner compiles and
    # executes real controls in this same verified scope, then execs the fixture.
    # Its work consumes, rather than extends, the caller's existing deadline.
    exec env -u LD_PRELOAD -u LD_LIBRARY_PATH \
        -u SEEN_FORK_SERIALIZER_TARGET -u SEEN_FORK_SERIALIZER_ROOT_PID \
        python3 "$arm_admission" --target-root "$canonical_target_root" \
        --compile-log "$canonical_compile_log" -- "$canonical_binary" "$@"
fi
ulimit -H -v "$SANITIZER_VMEM_LIMIT_KB" 2>/dev/null ||
    fail "the finite $shadow_label hard virtual-memory limit could not be applied"
active_hard_vmem_kb=$(ulimit -H -v 2>/dev/null || true)
[ "$active_hard_vmem_kb" = "$SANITIZER_VMEM_LIMIT_KB" ] ||
    fail "the finite $shadow_label hard virtual-memory limit failed exact read-back"
ulimit -S -v "$SANITIZER_VMEM_LIMIT_KB" 2>/dev/null ||
    fail "the finite $shadow_label virtual-memory allowance could not be applied"
active_soft_vmem_kb=$(ulimit -S -v 2>/dev/null || true)
[ "$active_soft_vmem_kb" = "$SANITIZER_VMEM_LIMIT_KB" ] ||
    fail "the finite $shadow_label virtual-memory allowance failed exact read-back"
# Preserve the exec'd fixture's stdout protocol byte-for-byte. Setup/readback
# evidence belongs on stderr, including when a supervisor pipes JSON checkpoints.
printf 'sanitizer-vmem soft_kb=%s hard_kb=%s\n' "$active_soft_vmem_kb" "$active_hard_vmem_kb" >&2
printf 'sanitizer-address-layout=%s %s\n' "$ADDRESS_LAYOUT" "$address_layout_options" >&2

if [ -n "$FIXTURE_PRELOAD" ]; then
    [ "$(sha256sum -- "$FIXTURE_PRELOAD" | cut -d ' ' -f1)" = "$FIXTURE_PRELOAD_SHA256" ] ||
        fail "fixture library changed after admission"
    printf 'sanitizer-fixture-preload sha256=%s path=%s\n' "$FIXTURE_PRELOAD_SHA256" "$FIXTURE_PRELOAD" >&2
fi


sanitizer_env=(
    -u LD_PRELOAD
    -u SEEN_FORK_SERIALIZER_TARGET
    -u SEEN_FORK_SERIALIZER_ROOT_PID
    -u LSAN_OPTIONS
    -u ASAN_SYMBOLIZER_PATH
    UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0
    TSAN_OPTIONS=halt_on_error=1:exitcode=66:report_signal_unsafe=1
)
asan_options="abort_on_error=1:halt_on_error=1:detect_leaks=1:symbolize=0:$address_layout_options"
if [ -z "$DRIVER_LEAK_PROFILE" ]; then
    exec env "${sanitizer_env[@]}" ASAN_OPTIONS="$asan_options" LSAN_OPTIONS=exitcode=23 \
        "${fixture_preload_env[@]}" "$canonical_binary" "$@"
fi

# Owner-approved scoped GPU driver leak attribution (FEL-1111). LSan reports
# without exiting so driver destructors still run; ASan errors still abort.
# The attribution owner, not LSan's exit code, then decides: only leaks
# allocated by approved driver modules, bounded by the same-run SDK-only
# control, are accepted. Every other leak exits 23 exactly as before.
reports=$(mktemp -d "$canonical_target_root/driver-leak-reports.XXXXXX") ||
    fail "driver leak report directory could not be created"
control_status=0
env "${sanitizer_env[@]}" ASAN_OPTIONS="$asan_options:log_path=$reports/control" LSAN_OPTIONS=exitcode=0 \
    "$DRIVER_LEAK_CONTROL" >/dev/null 2>"$reports/control.stderr" || control_status=$?
[ "$control_status" = 0 ] || { cat -- "$reports/control.stderr" >&2; fail "SDK-only $DRIVER_LEAK_PROFILE control failed with status $control_status"; }
fixture_status=0
env "${sanitizer_env[@]}" ASAN_OPTIONS="$asan_options:log_path=$reports/fixture" LSAN_OPTIONS=exitcode=0 \
    "${fixture_preload_env[@]}" "$canonical_binary" "$@" || fixture_status=$?
shopt -s nullglob
control_reports=("$reports"/control.[0-9]*)
fixture_reports=("$reports"/fixture.[0-9]*)
shopt -u nullglob
# Keep every fixture sanitizer report in the caller's log.
if [ "${#fixture_reports[@]}" -gt 0 ]; then cat -- "${fixture_reports[@]}" >&2; fi
[ "$fixture_status" = 0 ] || exit "$fixture_status"
python3 "$SCRIPT_DIR/gpu_driver_leak_attribution.py" --backend "$DRIVER_LEAK_PROFILE" \
    --control "${control_reports[@]}" --fixture "${fixture_reports[@]}" || exit 23
