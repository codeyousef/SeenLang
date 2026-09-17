#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-result-payload-pool-live
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/result-pool-live.XXXXXX")"

(
    set -x
    clang -O1 -g -fsanitize=address -fno-omit-frame-pointer \
    -I "$ROOT_DIR/seen_runtime" \
    "$ROOT_DIR/tests/fixtures/pool_live_reserve_runtime.c" \
    "$ROOT_DIR/seen_runtime/seen_runtime.c" -pthread -ldl -lm \
    -o "$WORK/pool-runtime-asan"
) >"$WORK/pool-runtime-compile.log" 2>&1
timeout --foreground --kill-after=5s 120s \
    bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
    --target-root "$WORK" --compile-log "$WORK/pool-runtime-compile.log" \
    -- "$WORK/pool-runtime-asan" \
    >"$WORK/pool-runtime.log" 2>&1 || {
        tail -c 32768 "$WORK/pool-runtime.log" >&2
        exit 1
    }
grep -Fq 'PASS: pooled live usage, recyclable reserve, physical budget' \
    "$WORK/pool-runtime.log"

for profile in fast release; do
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1)
    if [ "$profile" = fast ]; then
        flags+=(--fast)
    else
        flags+=(--release --lto=thin)
    fi
    for fixture in result_generic_payload_free pool_live_reserve_lifecycle; do
        source="$ROOT_DIR/tests/fixtures/$fixture.seen"
        binary="$WORK/$fixture-$profile"
        ir_dir="$WORK/$fixture-$profile-ir"
        log="$WORK/$fixture-$profile.log"
        timeout --foreground --kill-after=10s 900s \
            bash "$ATTESTED_SEEN" "$COMPILER" check "$source" \
            >"$log" 2>&1 || {
                tail -c 32768 "$log" >&2
                exit 1
            }
        timeout --foreground --kill-after=10s 900s \
            bash "$ATTESTED_SEEN" "$COMPILER" compile "$source" \
            "$binary" "${flags[@]}" --emit-module-ir-dir "$ir_dir" \
            >>"$log" 2>&1 || {
                tail -c 32768 "$log" >&2
                exit 1
            }
        "$ROOT_DIR/scripts/check_x86_executable_baseline.sh" \
            x86-64 "$binary" >>"$log" 2>&1
        timeout --foreground --kill-after=5s 120s "$binary" \
            >>"$log" 2>&1 || {
                tail -c 32768 "$log" >&2
                exit 1
            }
    done
    grep -Fq 'PASS: concrete generic Result payload release' \
        "$WORK/result_generic_payload_free-$profile.log"
    grep -Fq 'PASS: cold and warm JSON/SHA live usage excludes reserve' \
        "$WORK/pool_live_reserve_lifecycle-$profile.log"
    if rg --no-ignore -q '@T_free' \
        "$WORK/result_generic_payload_free-$profile-ir"; then
        echo 'FAIL: unresolved generic T_free remained in emitted IR' >&2
        exit 1
    fi
done
echo 'PASS: fast and release/ThinLTO generic Result and pool live usage'
