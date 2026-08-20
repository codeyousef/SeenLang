#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-atomic-text-io
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
ARTIFACT_ROOT="$SEEN_ARTIFACT_ROOT"

TEST_ROOT="$(mktemp -d "$ARTIFACT_ROOT/seen-atomic-text-io.XXXXXX")"
WINE_PREFIX="$TEST_ROOT/wine-prefix"
WINE_OVERRIDES="explorer.exe,services.exe,winemenubuilder.exe=d"
WINE_CPU_TOPOLOGY="1:1"
wine_server_started=0
cleanup() {
    status=$?
    if [ "$wine_server_started" = "1" ] && command -v wineserver >/dev/null 2>&1; then
        env WINEPREFIX="$WINE_PREFIX" wineserver -k >/dev/null 2>&1 || true
        env WINEPREFIX="$WINE_PREFIX" wineserver -w >/dev/null 2>&1 || true
    fi
    if [ -d "$TEST_ROOT/work/locked" ] && [ ! -L "$TEST_ROOT/work/locked" ]; then
        chmod 0700 "$TEST_ROOT/work/locked" 2>/dev/null || true
    fi
    case "$TEST_ROOT" in
        "$ARTIFACT_ROOT"/seen-atomic-text-io.*)
            if [ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ]; then
                rm -rf -- "$TEST_ROOT"
            fi
            ;;
        *)
            echo "refusing to remove unexpected test path: $TEST_ROOT" >&2
            status=1
            ;;
    esac
    exit "$status"
}
trap cleanup EXIT

run_helper_capped() {
    (
        if ! ulimit -S -v "$SEEN_OPT_VMEM_KB" 2>/dev/null; then
            echo "RESOURCE STOP: could not apply helper memory cap" >&2
            exit 126
        fi
        active_vmem=$(ulimit -S -v 2>/dev/null || true)
        case "$active_vmem" in
            ''|*[!0-9]*)
                echo "RESOURCE STOP: could not read back helper memory cap" >&2
                exit 126
                ;;
        esac
        [ "$active_vmem" -le "$SEEN_OPT_VMEM_KB" ] || {
            echo "RESOURCE STOP: helper memory cap read-back exceeds request" >&2
            exit 126
        }
        "$@"
    )
}

mkdir -p -- "$TEST_ROOT/work"
if [ ! -x "$COMPILER" ]; then
    echo "missing executable checkout compiler: $COMPILER" >&2
    exit 1
fi
check_output="$TEST_ROOT/seen-check.log"
if ! (NO_COLOR=1 SEEN_COLOR=never SEEN_DATA_PATH="$ROOT_DIR/languages" \
    bash "$ATTESTED_SEEN" "$COMPILER" check \
        "$ROOT_DIR/tests/misc_root_tests/seen_atomic_text_io_api.seen") \
        >"$check_output" 2>&1; then
    cat "$check_output" >&2
    exit 1
fi
if grep -Eiq '(^|[^[:alpha:]])error(\[[^]]+\])?:' "$check_output"; then
    cat "$check_output" >&2
    echo "Seen atomic text I/O API fixture reported a diagnostic" >&2
    exit 1
fi
run_helper_capped clang -std=c11 -O1 -pthread \
        -DSEEN_TEST_ATOMIC_IO_FAILURE_INJECTION \
        -I "$ROOT_DIR/seen_runtime" \
        "$ROOT_DIR/tests/misc_root_tests/seen_atomic_text_io_test.c" \
        "$ROOT_DIR/seen_runtime/seen_runtime.c" \
        -ldl -lm -o "$TEST_ROOT/seen-atomic-text-io-test"

"$TEST_ROOT/seen-atomic-text-io-test" "$TEST_ROOT/work"

# Compile the Windows branch when a cross compiler is available. Execution is
# covered on Windows CI; this local gate still catches Win32 API/type drift.
if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    run_helper_capped x86_64-w64-mingw32-gcc -std=c11 -O1 \
            -DSEEN_TEST_ATOMIC_IO_FAILURE_INJECTION \
            -ffunction-sections -fdata-sections \
            -I "$ROOT_DIR/seen_runtime" \
            "$ROOT_DIR/tests/misc_root_tests/seen_atomic_text_io_windows_test.c" \
            "$ROOT_DIR/seen_runtime/seen_runtime.c" \
            -Wl,--gc-sections -lkernel32 -ladvapi32 -lshell32 -lws2_32 \
            -o "$TEST_ROOT/seen-atomic-text-io-windows.exe"
    if command -v wine >/dev/null 2>&1; then
        if ! command -v taskset >/dev/null 2>&1; then
            echo "Wine execution requires taskset for bounded CPU topology" >&2
            exit 1
        fi
        wine_cpu=""
        while read -r cpu_key cpu_value; do
            if [ "$cpu_key" = "Cpus_allowed_list:" ]; then
                wine_cpu="${cpu_value%%[-,]*}"
                break
            fi
        done < /proc/self/status
        case "$wine_cpu" in
            ''|*[!0-9]*)
                echo "could not select an allowed CPU for bounded Wine execution" >&2
                exit 1
                ;;
        esac
        mkdir -p -- "$TEST_ROOT/wine-home" "$TEST_ROOT/wine-prefix" \
            "$TEST_ROOT/wine-cache" "$TEST_ROOT/wine-config" \
            "$TEST_ROOT/wine-data"
        if [ -n "${SEEN_WINE_PREFIX_TEMPLATE:-}" ]; then
            case "$SEEN_WINE_PREFIX_TEMPLATE" in
                "$ARTIFACT_ROOT"/*) ;;
                *)
                    echo "Wine prefix template escaped the artifact root" >&2
                    exit 1
                    ;;
            esac
            if [ ! -d "$SEEN_WINE_PREFIX_TEMPLATE" ] ||
                [ -L "$SEEN_WINE_PREFIX_TEMPLATE" ]; then

                echo "Wine prefix template is not a safe directory" >&2
                exit 1
            fi
            cp -a -- "$SEEN_WINE_PREFIX_TEMPLATE/." "$WINE_PREFIX/"
        fi
        wine_server_started=1
        for windows_run in 1 2 3; do
            windows_work="$TEST_ROOT/windows-work-$windows_run"
            mkdir -p -- "$windows_work"
            (
                cd "$windows_work"
                env \
                    HOME="$TEST_ROOT/wine-home" \
                    XDG_CACHE_HOME="$TEST_ROOT/wine-cache" \
                    XDG_CONFIG_HOME="$TEST_ROOT/wine-config" \
                    XDG_DATA_HOME="$TEST_ROOT/wine-data" \
                    WINEARCH=win64 \
                    WINEPREFIX="$WINE_PREFIX" \
                    WINEDEBUG=-all \
                    WINEDLLOVERRIDES="$WINE_OVERRIDES" \
                    WINE_CPU_TOPOLOGY="$WINE_CPU_TOPOLOGY" \
                    taskset -c "$wine_cpu" \
                    wine "$TEST_ROOT/seen-atomic-text-io-windows.exe"
            )
            if [ ! -f "$windows_work/windows-test-passed.marker" ] ||
                [ "$(tr -d '\r\n' < \
                    "$windows_work/windows-test-passed.marker")" != \
                    "passed" ]; then
                echo "Windows atomic text I/O test run $windows_run did not reach completion" >&2
                exit 1
            fi
        done
        env WINEPREFIX="$WINE_PREFIX" wineserver -w
        wine_server_started=0
    fi
fi

echo "atomic text I/O tests passed"
