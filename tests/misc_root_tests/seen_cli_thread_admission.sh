#!/usr/bin/env bash
# LLVM 20 cannot execute TSan under the reviewed finite-hard-address policy.
# A CLI profile must not turn an unadmitted generated execution into a pass.
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
python3 - "$SEEN_ARTIFACT_ROOT" "$TMP_DIR/before.json" <<'PY'
import json, sys
from pathlib import Path
root = Path(sys.argv[1])
Path(sys.argv[2]).write_text(json.dumps(sorted(str(p) for p in root.glob('test-cli-thread.*'))))
PY
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
if sys.argv[2] == 'ci':
    assert int(sys.argv[3]) == 0 and report['passed'] == 1 and report['failed'] == 0, report
    assert selected[0]['exit_code'] == 0, selected
else:
    assert int(sys.argv[3]) != 0 and report['passed'] == 0 and report['failed'] == 1, report
    assert selected[0]['exit_code'] != 0, selected
# Execution logs count selected cases, not skipped report rows.
execution_log = Path(sys.argv[4]) / '.seen_cache/test/run/test-0.log'
if execution_log.is_file() and not execution_log.is_symlink():
    with execution_log.open('rb') as stream:
        stream.seek(max(0, execution_log.stat().st_size - 32768))
        Path(sys.argv[5]).write_bytes(stream.read(32768))
PY
done
# Require a freshly linked TSan ELF as well as the failed execution result:
# invalid source, stale metadata or a compiler failure cannot satisfy this case.
python3 - "$SEEN_ARTIFACT_ROOT" "$TMP_DIR/before.json" "$TMP_DIR/native-images.json" <<'PY'
import itertools, json, stat, subprocess, sys
from pathlib import Path
root = Path(sys.argv[1])
before = set(json.loads(Path(sys.argv[2]).read_text()))
fresh = [p for p in root.glob('test-cli-thread.*') if str(p) not in before]
assert len(fresh) == 1, fresh
entries = list(itertools.islice(fresh[0].iterdir(), 33))
assert len(entries) <= 32, entries
images = []
for path in entries:
    mode = path.lstat().st_mode
    if stat.S_ISREG(mode) and mode & 0o111:
        result = subprocess.run(['readelf', '-Ws', str(path)], check=True, text=True, capture_output=True, timeout=30)
        assert '__tsan_init' in result.stdout, path
        images.append(str(path))
assert len(images) == 1, images
Path(sys.argv[3]).write_text(json.dumps({'schema': 'seen-cli-thread-admission-regression-v1', 'linked_tsan_images': images, 'execution_passed': False}, indent=2) + '\n')
PY
printf 'PASS: native CLI keeps ordinary execution and refuses unadmitted LLVM 20 thread-profile success\n'
