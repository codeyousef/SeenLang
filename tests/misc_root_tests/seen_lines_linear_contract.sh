#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SEEN_BIN=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-lines-linear-contract
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$SEEN_BIN" -- bash "$0" "$@"
fi
SEEN_BIN="${SEEN_CAPPED_REGRESSION_COMPILER:-$SEEN_BIN}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$SEEN_BIN"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
RELEASE_TARGET_CPU=${SEEN_RELEASE_CPU_BASELINE:-x86-64}
case "$RELEASE_TARGET_CPU" in
    x86-64|x86-64-v3) ;;
    *)
        echo "FAIL: unsupported lines contract CPU baseline: $RELEASE_TARGET_CPU" >&2
        exit 2
        ;;
esac
BASELINE_CHECKER="$ROOT_DIR/scripts/check_x86_executable_baseline.sh"
SOURCE="$ROOT_DIR/tests/fixtures/fel-1536/lines_contract.seen"
WORK_DIR=$(mktemp -d "$SEEN_ARTIFACT_ROOT/lines-linear-contract.XXXXXX")
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ]; then
        case "$WORK_DIR" in
            "$SEEN_ARTIFACT_ROOT"/lines-linear-contract.*)
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    [ "$(dirname -- "$WORK_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
                rm -rf -- "$WORK_DIR" ;;
            *) return 1 ;;
        esac
    else echo "Preserved failed FEL-1536 artifacts: $WORK_DIR" >&2; fi
}
trap cleanup EXIT

run_seen() { bash "$ATTESTED_SEEN" "$SEEN_BIN" "$@"; }

run_seen check "$SOURCE" >"$WORK_DIR/check.log" 2>&1
run_seen compile "$SOURCE" "$WORK_DIR/fast" --fast --no-cache --no-fork \
    --target-cpu "$RELEASE_TARGET_CPU" \
    >"$WORK_DIR/fast.log" 2>&1
run_seen compile "$SOURCE" "$WORK_DIR/release" --release --lto thin --no-fork \
    --no-cache --target-cpu "$RELEASE_TARGET_CPU" \
    >"$WORK_DIR/release.log" 2>&1

bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/fast"
bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/release"
timeout --kill-after=5s 120s bash -c 'ulimit -S -s 8192; exec "$@"' bash "$WORK_DIR/fast"

counts=(61897 123794 247587)
times=()
for count in "${counts[@]}"; do
    corpus="$WORK_DIR/lines-$count.txt"
    awk -v count="$count" 'BEGIN {
        longer = int(count * 134628 / 247587)
        for (i = 0; i < count; i++) {
            if (i < longer) print "αβγxxxxxxx"
            else print "αβγxxxxxx"
        }
    }' \
        >"$corpus"
    if [ "$count" -eq 247587 ]; then
        [ "$(wc -c <"$corpus")" -eq 3353259 ] || {
            echo "FAIL: production corpus byte size drifted" >&2
            exit 1
        }
        [ "$(wc -l <"$corpus")" -eq 247587 ] || {
            echo "FAIL: production corpus line count drifted" >&2
            exit 1
        }
    fi
    elapsed=$(timeout 60 bash -c 'ulimit -S -s 8192; exec "$@"' bash \
        "$WORK_DIR/release" "$corpus" "$count")
    case "$elapsed" in ''|*[!0-9]*) echo "FAIL: invalid timing evidence" >&2; exit 1;; esac
    times+=("$elapsed")
done

awk -v a="${times[0]}" -v b="${times[1]}" -v c="${times[2]}" 'BEGIN {
    floor = 1000000
    if (a < floor) a = floor
    if (b < floor) b = floor
    if (b > a * 4 || c > b * 4) exit 1
}' || { echo "FAIL: lines scaling is superlinear" >&2; exit 1; }

printf 'PASS: FEL-1536 lines scaling ns=%s,%s,%s\n' \
    "${times[0]}" "${times[1]}" "${times[2]}"
