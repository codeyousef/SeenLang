#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-structured-contract
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
[ "${SEEN_LOW_MEMORY:-0}" = 1 ] && [ "${SEEN_JOBS:-0}" = 1 ] &&
    [ "${SEEN_OPT_JOBS:-0}" = 1 ] || {
    echo "FAIL: ASYNC-001A-G requires serial low-memory settings" >&2
    exit 126
}

ASYNC_ROOT="$ROOT_DIR/seen_std/src/async"
TEST_ROOT="$ROOT_DIR/seen_std/tests/async"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/async-contract.XXXXXX")"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$WORK_DIR" in
            "$SEEN_ARTIFACT_ROOT"/async-contract.*)
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
                ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved async artifacts: $WORK_DIR" >&2
    fi
    exit "$status"
}
trap cleanup EXIT

for module in structured mod; do
    [ -f "$ASYNC_ROOT/$module.seen" ] && [ ! -L "$ASYNC_ROOT/$module.seen" ] || {
        echo "FAIL: missing native Seen async module $module" >&2
        exit 1
    }
done
[ ! -e "$ASYNC_ROOT/runtime.seen" ] && [ ! -e "$ASYNC_ROOT/future.seen" ] || {
    echo "FAIL: conflicting poll-all async fallback remains packaged" >&2
    exit 1
}
grep -Fq 'src/async/structured.seen' "$ROOT_DIR/seen_std/Seen.toml" || {
    echo "FAIL: structured async is absent from the stdlib manifest" >&2
    exit 1
}
grep -Fq 'seen-structured-async-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || {
    echo "FAIL: structured async compatibility identity is missing" >&2
    exit 1
}
grep -Fq 'seen-structured-async-v1' "$ROOT_DIR/CHANGELOG.md" || {
    echo "FAIL: structured async changelog entry is missing" >&2
    exit 1
}

for symbol in OperationContext CancellationSource ReadyQueue 'JoinHandle<T>' \
    'Scope<T>' 'DeterministicExecutor<T>' selectTaskId timeoutJoin \
    boundedFanOut AsyncWaitGroup AsyncBarrier deterministicReduceSum \
    async.invalid async.limit async.cancelled async.timeout async.unavailable \
    async.unsupported_platform; do
    rg -Fq "$symbol" "$ASYNC_ROOT" || {
        echo "FAIL: missing structured-async contract $symbol" >&2
        exit 1
    }
done

rg -Fq 'let next = this.ready.pop()' "$ASYNC_ROOT/structured.seen" || {
    echo "FAIL: executor is not driven by an explicit ready wake" >&2
    exit 1
}
if rg -n 'while .*tasks\.length\(\).*tick|poll all|poll_all' \
    "$ASYNC_ROOT/structured.seen"; then
    echo "FAIL: executor contains a dormant-task polling scan" >&2
    exit 1
fi

for issue in a b c d e f g; do
    upper="${issue^^}"
    source_file="$TEST_ROOT/async-001${issue}.seen"
    output_file="$WORK_DIR/async-001${issue}"
    log_file="$WORK_DIR/async-001${issue}.log"
    [ -f "$source_file" ] && [ ! -L "$source_file" ] || {
        echo "FAIL: missing ASYNC-001$upper test" >&2
        exit 1
    }
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "ASYNC-001${upper}_${case_name}" "$source_file" || {
            echo "FAIL: ASYNC-001$upper missing $case_name case" >&2
            exit 1
        }
    done
    bash "$ATTESTED_SEEN" "$COMPILER" compile "$source_file" "$output_file" --no-cache \
        --target-cpu=x86-64 --jobs 1 --opt-jobs 1 --frozen \
        >"$log_file" 2>&1
    timeout --foreground --kill-after=5s 60s "$output_file" >>"$log_file" 2>&1
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "PASS: ASYNC-001${upper}_${case_name}" "$log_file" || {
            echo "FAIL: ASYNC-001$upper did not pass $case_name" >&2
            exit 1
        }
    done
done

[ -z "$(jobs -pr)" ] || {
    echo "FAIL: async cleanup left background jobs" >&2
    exit 1
}
echo "PASS: ASYNC-001A-G structured concurrency contract"
