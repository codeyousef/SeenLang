#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
RUNNER="$ROOT_DIR/scripts/run_with_project_artifacts.sh"
SOURCE="$ROOT_DIR/tests/fixtures/fel-1536/lines_contract.seen"
mkdir -p "$ROOT_DIR/.seen/agent-tools/fel-1536"
WORK_DIR=$(mktemp -d "$ROOT_DIR/.seen/agent-tools/fel-1536/run.XXXXXX")
cleanup() {
    status=$?
    if [ "$status" -eq 0 ]; then rm -rf "$WORK_DIR"
    else echo "Preserved failed FEL-1536 artifacts: $WORK_DIR" >&2; fi
}
trap cleanup EXIT

run_seen() { "$RUNNER" fel-1536 --keep-on-failure -- "$SEEN_BIN" "$@"; }

run_seen check "$SOURCE" >"$WORK_DIR/check.log" 2>&1
run_seen compile "$SOURCE" "$WORK_DIR/fast" >"$WORK_DIR/fast.log" 2>&1
run_seen compile "$SOURCE" "$WORK_DIR/release" --release --lto thin --no-fork \
    >"$WORK_DIR/release.log" 2>&1

"$WORK_DIR/fast"

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
