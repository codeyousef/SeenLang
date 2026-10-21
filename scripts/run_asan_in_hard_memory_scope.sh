#!/usr/bin/env bash
# Run one generated Linux x86-64 ASan/TSan executable with enough finite virtual
# address space for the sanitizer shadow. Physical memory, swap, task, worker,
# and timeout containment remain owned by the enclosing read-back-verified
# capped-regression scope.

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
while [ "$#" -gt 0 ]; do
    case "$1" in
        --sanitizer)
            [ "$#" -ge 2 ] || fail "--sanitizer requires a value"
            SANITIZER=$2
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
[ -n "$TARGET_ROOT" ] && [ -n "$COMPILE_LOG" ] && [ "$#" -ge 1 ] ||
    fail "usage: $0 --target-root DIR --compile-log FILE -- /absolute/generated/executable [args...]"

[ "$(uname -s)" = "Linux" ] && [ "$(uname -m)" = "x86_64" ] ||
    fail "the finite ASan virtual-address allowance is certified only on Linux x86-64"

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
timeout 10 readelf -Ws -- "$canonical_binary" 2>/dev/null |
    awk -v symbol="$runtime_symbol" '{ split($NF, name, "@"); if (name[1] == symbol) found = 1 }
        END { exit(found ? 0 : 1) }' ||
    fail "the generated executable has no $sanitizer_label runtime symbol"

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

exec env \
    -u LD_PRELOAD \
    -u SEEN_FORK_SERIALIZER_TARGET \
    -u SEEN_FORK_SERIALIZER_ROOT_PID \
    -u LSAN_OPTIONS \
    -u ASAN_SYMBOLIZER_PATH \
    ASAN_OPTIONS=abort_on_error=1:halt_on_error=1:detect_leaks=1:symbolize=0 \
    LSAN_OPTIONS=exitcode=23 \
    UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
    TSAN_OPTIONS=halt_on_error=1:exitcode=66:report_signal_unsafe=1 \
    "$canonical_binary" "$@"
