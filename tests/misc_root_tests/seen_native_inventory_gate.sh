#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_native_inventory.py"
INVENTORY="$ROOT_DIR/docs/architecture/native-inventory.json"
HAPPY="$ROOT_DIR/tests/fixtures/p0-arch-002/happy"
INVALID="$ROOT_DIR/tests/fixtures/p0-arch-002/invalid"
EXPECTED="$HAPPY/expected.json"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: native inventory gate: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init native-inventory-tests) ||
    fail "could not initialize test scope"
TEST_ROOT=$(seen_artifact_mktemp_dir "$test_scope" run) ||
    fail "could not create test root"
[ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] || fail "unsafe test root"

cleanup() {
    local status=$?
    case "$TEST_ROOT" in
        "$test_scope"/run.*)
            [ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] &&
                [ "${TEST_ROOT%/*}" = "$test_scope" ] || return 1
            rm -rf -- "$TEST_ROOT" || return 1
            ;;
        *) return 1 ;;
    esac
    return "$status"
}
trap cleanup EXIT

python3 "$CHECKER" --root "$ROOT_DIR" --check "$INVENTORY" ||
    fail "production inventory is stale"

python3 "$CHECKER" --root "$HAPPY" > "$TEST_ROOT/happy-a.json" ||
    fail "P0-ARCH-002_happy"
python3 "$CHECKER" --root "$HAPPY" > "$TEST_ROOT/happy-b.json" ||
    fail "P0-ARCH-002_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "P0-ARCH-002_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$EXPECTED" ||
    fail "P0-ARCH-002_happy bytes changed"

if python3 "$CHECKER" --root "$INVALID" > "$TEST_ROOT/invalid.json" 2>/dev/null; then
    fail "P0-ARCH-002_invalid was accepted"
fi
if python3 "$CHECKER" --root "$HAPPY" --max-files 1 >/dev/null 2>&1; then
    fail "P0-ARCH-002_limit was accepted"
fi

set +e
SEEN_NATIVE_INVENTORY_TEST_HOOKS=1 python3 "$CHECKER" --root "$HAPPY" \
    --test-cancel-after-files 1 > "$TEST_ROOT/cancel.json" 2> "$TEST_ROOT/cancel.err"
cancel_status=$?
set -e
[ "$cancel_status" -eq 130 ] || fail "P0-ARCH-002_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] || fail "P0-ARCH-002_cancel emitted partial JSON"

printf '%s\n' sentinel > "$TEST_ROOT/cleanup.json"
if python3 "$CHECKER" --root "$INVALID" --write "$TEST_ROOT/cleanup.json" \
    >/dev/null 2>&1; then
    fail "P0-ARCH-002_cleanup invalid write succeeded"
fi
[ "$(< "$TEST_ROOT/cleanup.json")" = sentinel ] ||
    fail "P0-ARCH-002_cleanup replaced the destination"
if compgen -G "$TEST_ROOT/.cleanup.json.*" >/dev/null; then
    fail "P0-ARCH-002_cleanup leaked an atomic-write temporary"
fi

echo "PASS: deterministic native inventory gate"
