#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-runtime-cstring-accounting
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/runtime-cstring.XXXXXX")"

if rg -n 'free\((cpath|cmode|csource|cdestination|ccmd|cname|cvalue|crequest|root_input|path_input)\)' \
    "$ROOT_DIR/seen_runtime/seen_runtime.c" >"$WORK/raw-cstring-free.log"; then
    cat "$WORK/raw-cstring-free.log" >&2
    echo 'FAIL: a seen_runtime_cstring caller still uses raw free' >&2
    exit 1
fi

(
    set -x
    clang -O1 -g -fsanitize=address -fno-omit-frame-pointer \
        -I "$ROOT_DIR/seen_runtime" \
        "$ROOT_DIR/tests/fixtures/runtime_cstring_accounting.c" \
        "$ROOT_DIR/seen_runtime/seen_runtime.c" -pthread -ldl -lm \
        -o "$WORK/runtime-cstring-asan"
) >"$WORK/runtime-cstring-compile.log" 2>&1
timeout --foreground --kill-after=5s 180s \
    bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
    --target-root "$WORK" --compile-log "$WORK/runtime-cstring-compile.log" \
    -- "$WORK/runtime-cstring-asan" "$WORK" \
    >"$WORK/runtime-cstring.log" 2>&1 || {
        tail -c 32768 "$WORK/runtime-cstring.log" >&2
        exit 1
    }
grep -Fq 'PASS: runtime C-string callers restore live accounting' \
    "$WORK/runtime-cstring.log"

mkdir -p "$WORK/runtime-cstring-évidence"
truncate -s 530577 \
    "$WORK/runtime-cstring-évidence/locked_q4_manifest.json"
for profile in fast release; do
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1)
    if [ "$profile" = fast ]; then
        flags+=(--fast)
    else
        flags+=(--release --lto=thin)
    fi
    binary="$WORK/read-text-$profile"
    log="$WORK/read-text-$profile.log"
    timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" check \
        "$ROOT_DIR/tests/fixtures/runtime_cstring_read_text.seen" \
        >"$log" 2>&1 || {
            tail -c 32768 "$log" >&2
            exit 1
        }
    timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$ROOT_DIR/tests/fixtures/runtime_cstring_read_text.seen" "$binary" \
        "${flags[@]}" >>"$log" 2>&1 || {
            tail -c 32768 "$log" >&2
            exit 1
        }
    "$ROOT_DIR/scripts/check_x86_executable_baseline.sh" x86-64 "$binary" \
        >>"$log" 2>&1
    timeout --foreground --kill-after=5s 180s \
        bash -c 'cd "$1" && exec "$2"' bash "$WORK" "$binary" \
        >>"$log" 2>&1 || {
            tail -c 32768 "$log" >&2
            exit 1
        }
    grep -Fq 'readText live/reserve baseline/after=0/0/0/0' "$log"
done

echo 'PASS: fast and release/ThinLTO runtime C-string accounting'
