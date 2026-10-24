#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-compiled-fixture-metadata
COMPILER="${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/fixture-metadata.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=fixture-metadata
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fail() { printf 'FAIL: compiled fixture metadata: %s\n' "$*" >&2; exit 1; }

compile_fixture fast probe "$ROOT_DIR/tests/fixtures/test-002a/fixture_metadata_probe.seen"

copy_probe() {
    local name=$1 suffix
    for suffix in '' .log .compile.log .compile.status; do
        cp -- "$TMP_DIR/probe-fast$suffix" "$TMP_DIR/$name-fast$suffix"
    done
    cmp -- "$TMP_DIR/probe-fast" "$TMP_DIR/$name-fast"
}

copy_probe clone
run_fixture fast clone
grep -Fxq 'PASS: fixture metadata actual executable' "$TMP_DIR/clone-fast.log"

for missing in log status; do
    name="missing-$missing"
    copy_probe "$name"
    rm -- "$TMP_DIR/$name-fast.compile.$missing"
    status=0
    run_fixture fast "$name" >"$TMP_DIR/$name.rejection.log" 2>&1 || status=$?
    expected=1
    if [ "$missing" = status ]; then expected=126; fi
    [ "$status" -eq "$expected" ] || fail "$name returned $status, expected $expected"
    [ ! -e "$TMP_DIR/$name-fast.run.status" ] || fail "$name reached execution"
done

copy_probe invalid-status
printf 'not-a-status\n' > "$TMP_DIR/invalid-status-fast.compile.status"
status=0
run_fixture fast invalid-status >"$TMP_DIR/invalid-status.rejection.log" 2>&1 || status=$?
[ "$status" -eq 126 ] || fail 'malformed compile status was accepted'
[ ! -e "$TMP_DIR/invalid-status-fast.run.status" ] || fail 'malformed status reached execution'

copy_probe missing-binary
rm -- "$TMP_DIR/missing-binary-fast"
status=0
run_fixture fast missing-binary >"$TMP_DIR/missing-binary.rejection.log" 2>&1 || status=$?
[ "$status" -eq 127 ] || fail 'stale successful metadata hid a missing executable'

status=0
compile_fixture fast rejected "$ROOT_DIR/tests/fixtures/test-002a/fixture_metadata_invalid.seen" || status=$?
[ "$status" -ne 0 ] || fail 'invalid source compiled successfully'
[ ! -e "$TMP_DIR/rejected-fast" ] || fail 'failed compilation produced an executable'
grep -Fq 'missingFixtureMetadataValue' "$TMP_DIR/rejected-fast.compile.log"
compiled_status=$status
status=0
run_fixture fast rejected >"$TMP_DIR/rejected.rejection.log" 2>&1 || status=$?
[ "$status" -eq "$compiled_status" ] || fail 'failed compile status was not preserved'
[ ! -e "$TMP_DIR/rejected-fast.run.status" ] || fail 'rejected compilation reached execution'

run_fixture fast clone
grep -Fxq 'PASS: fixture metadata actual executable' "$TMP_DIR/clone-fast.log"
echo 'PASS: real compiled fixture clones preserve evidence and reject missing, malformed and stale metadata'
