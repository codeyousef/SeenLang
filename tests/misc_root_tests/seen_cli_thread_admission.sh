#!/usr/bin/env bash
# Require admitted CLI execution and real Seen clean/racy native worker bodies.
# A compiler failure, stock-runtime startup failure, or timeout cannot pass.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-cli-thread-admission
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/cli-thread-admission.XXXXXX")"
relative="${TMP_DIR#"$ROOT_DIR/"}"
[ "$relative" != "$TMP_DIR" ] || exit 126
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in "$SEEN_ARTIFACT_ROOT"/cli-thread-admission.*) rm -rf -- "$TMP_DIR" ;; *) status=1 ;; esac
    else
        printf 'Preserved CLI thread admission evidence: %s\n' "$TMP_DIR" >&2
    fi
    exit "$status"
}
trap cleanup EXIT
python3 "$ROOT_DIR/scripts/finite_tsan_sdk.py" validate \
    "${SEEN_TSAN_SDK_MANIFEST:?finite TSan SDK must be explicitly provisioned}" \
    --manifest-sha256 "${SEEN_TSAN_SDK_MANIFEST_SHA256:?SDK publication digest required}" >/dev/null
for profile in ci sanitizer-thread; do
    status=0
    timeout --foreground --kill-after=5s 900s env SEEN_KEEP_TMP=1 \
        bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" test "$ROOT_DIR" \
        --filter test_002a_instrumentation --profile "$profile" --jobs 1 --timeout 10m \
        --report "json:$relative/$profile.json" \
        > "$TMP_DIR/$profile.log" 2>&1 || status=$?
    printf '%s\n' "$status" > "$TMP_DIR/$profile.status"
    python3 "$ROOT_DIR/scripts/check_test_reporters.py" --format json \
        --validate "$TMP_DIR/$profile.json" >/dev/null
    python3 - "$TMP_DIR/$profile.json" "$profile" "$status" "$ROOT_DIR" "$TMP_DIR/$profile.execution.log" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(open(sys.argv[1]).read())
selected = [row for row in report['results'] if row['status'] != 'skipped']
assert len(selected) == 1 and selected[0]['path'] == 'compiler_seen/tests/test_002a_instrumentation.seen', report
assert int(sys.argv[3]) == 0 and report['passed'] == 1 and report['failed'] == 0, report
assert selected[0]['exit_code'] == 0, selected
# Execution logs count selected cases, not skipped report rows.
execution_log = Path(sys.argv[4]) / '.seen_cache/test/run/test-0.log'
if execution_log.is_file() and not execution_log.is_symlink():
    with execution_log.open('rb') as stream:
        stream.seek(max(0, execution_log.stat().st_size - 32768))
        Path(sys.argv[5]).write_bytes(stream.read(32768))
PY
done
WORKER_PROJECT="$TMP_DIR/worker-project"
mkdir -p "$WORKER_PROJECT/native"
cp "$ROOT_DIR/tests/compiler_regressions/tsan_seen_workers.seen" "$WORKER_PROJECT/main.seen"
cat >"$WORKER_PROJECT/Seen.toml" <<'TOML'
manifest-version = 1
[project]
name = "tsan_seen_workers"
version = "0.1.0"
[native.dependencies]
seen_tsan_workers_control = { path = "native" }
TOML
# Coordination is lock-free and never accesses the counter. Keep it sanitized;
# Result allocation and the normal handle registry may otherwise order stores.
timeout --foreground --kill-after=5s 60s clang --no-default-config -std=c11 \
    -O1 -g -fPIC -pthread -fsanitize=thread -fno-omit-frame-pointer \
    -Wall -Wextra -Werror -c \
    "$ROOT_DIR/tests/compiler_regressions/tsan_seen_workers_control.c" \
    -o "$WORKER_PROJECT/native/control.o" >"$TMP_DIR/control-compile.log" 2>&1
ar rcs "$WORKER_PROJECT/native/libseen_tsan_workers_control.a" \
    "$WORKER_PROJECT/native/control.o"
bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" compile \
    "$WORKER_PROJECT/main.seen" "$TMP_DIR/seen-workers" \
    --fast --no-cache --jobs 1 --opt-jobs 1 \
    --sanitize thread >"$TMP_DIR/workers-compile.log" 2>&1
# Mandatory fresh-process samples, not retries: any failed pair stops the gate.
SAMPLES=8
for ((sample=1; sample<=SAMPLES; sample++)); do
    SAMPLE_DIR="$TMP_DIR/native-$sample"
    mkdir "$SAMPLE_DIR"
    for mode in clean race; do
        status=0
        timeout --foreground --kill-after=5s 30s \
            bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --sanitizer thread --target-root "$TMP_DIR" \
            --compile-log "$TMP_DIR/workers-compile.log" -- "$TMP_DIR/seen-workers" "$mode" \
            >"$SAMPLE_DIR/workers-$mode.log" 2>&1 || status=$?
        printf '%s\n' "$status" >"$SAMPLE_DIR/workers-$mode.status"
    done
    python3 - "$SAMPLE_DIR" <<'PY'
import json, re, sys
from pathlib import Path
root = Path(sys.argv[1])
clean = (root / 'workers-clean.log').read_text()
race = (root / 'workers-race.log').read_text()
clean_status = int((root / 'workers-clean.status').read_text())
race_status = int((root / 'workers-race.status').read_text())
assert clean_status == 0, (clean_status, clean)
assert 'WARNING: ThreadSanitizer' not in clean, clean
for field, expected in [('WORKERS', 2), ('DISTINCT_NATIVE_WORKERS', 2), ('COUNTER', 2), ('DRAINED', 1)]:
    assert re.search(rf'^TSAN_SEEN_{field}={expected}$', clean, re.M), clean
assert race_status not in (0, 124, 137), (race_status, race)
assert 'WARNING: ThreadSanitizer: data race' in race, race
address = re.search(r'^TSAN_SEEN_COUNTER_ADDRESS=([0-9]+)$', race, re.M)
assert address, race
counter = int(address[1])
accesses = re.findall(r'\b(?:Read|Write|Previous read|Previous write) of size 8 at (0x[0-9a-fA-F]+) by thread (T[0-9]+)', race)
counter_threads = {thread for value, thread in accesses if int(value, 16) == counter}
assert len(counter_threads) >= 2, race
(root / 'native-workers.json').write_text(json.dumps({
    'schema': 'seen-tsan-native-workers-v1', 'clean_exit': clean_status,
    'race_exit': race_status, 'counter_address': address[1],
    'clean_workers': 2, 'clean_counter': 2, 'native_resources_drained': True,
    'distinct_native_workers': 2, 'race_threads': sorted(counter_threads),
    'seen_counter_race_detected': True,
}, indent=2) + '\n')
PY
done
python3 - "$TMP_DIR" "$SAMPLES" <<'PY'
import json, sys
from pathlib import Path
root = Path(sys.argv[1])
samples = []
for number in range(1, int(sys.argv[2]) + 1):
    value = json.loads((root / f'native-{number}/native-workers.json').read_text())
    del value['schema']
    samples.append({'sample': number, **value})
(root / 'native-workers.json').write_text(json.dumps({
    'schema': 'seen-tsan-native-workers-v2', 'samples': samples,
}, indent=2) + '\n')
PY
printf 'PASS: admitted CLI profiles and real Seen two-worker clean/race controls\n'
