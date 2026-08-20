#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_ci_containment.py"
PRODUCTION="$ROOT_DIR/docs/architecture/ci-containment.json"
SCHEMA="$ROOT_DIR/schemas/ci-containment.schema.json"
HAPPY="$ROOT_DIR/tests/fixtures/core-001b/happy"
INVALID="$ROOT_DIR/tests/fixtures/core-001b/invalid"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: CI containment contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init ci-containment-contract-tests) ||
    fail "could not initialize test scope"
TEST_ROOT=$(seen_artifact_mktemp_dir "$test_scope" run) ||
    fail "could not create test root"

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

python3 -m json.tool "$SCHEMA" >/dev/null || fail "schema JSON"
python3 "$CHECKER" "$PRODUCTION" > "$TEST_ROOT/production.json" ||
    fail "production policy"
python3 "$CHECKER" "$HAPPY/ci-containment.json" > "$TEST_ROOT/happy-a.json" ||
    fail "CORE-001B_happy"
python3 "$CHECKER" "$HAPPY/ci-containment.json" > "$TEST_ROOT/happy-b.json" ||
    fail "CORE-001B_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-001B_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$HAPPY/expected.json" ||
    fail "CORE-001B_happy bytes changed"

if python3 "$CHECKER" "$INVALID/ci-containment.json" \
    > "$TEST_ROOT/invalid.json" 2> "$TEST_ROOT/invalid.err"; then

    fail "CORE-001B_invalid was accepted"
fi
grep -Fq 'core.001b.invalid' "$TEST_ROOT/invalid.err" ||
    fail "CORE-001B_invalid omitted its typed diagnostic"

if python3 "$CHECKER" "$HAPPY/ci-containment.json" --max-bytes 1 \
    >/dev/null 2> "$TEST_ROOT/limit.err"; then

    fail "CORE-001B_limit was accepted"
fi
grep -Fq 'core.001b.limit' "$TEST_ROOT/limit.err" ||
    fail "CORE-001B_limit omitted its typed diagnostic"

set +e
SEEN_CI_CONTAINMENT_TEST_HOOKS=1 python3 "$CHECKER" \
    "$HAPPY/ci-containment.json" --test-cancel-after-read \
    > "$TEST_ROOT/cancel.json" 2> "$TEST_ROOT/cancel.err"
cancel_status=$?
set -e
[ "$cancel_status" -eq 130 ] || fail "CORE-001B_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] || fail "CORE-001B_cancel emitted partial JSON"
grep -Fq 'core.001b.cancelled' "$TEST_ROOT/cancel.err" ||
    fail "CORE-001B_cancel omitted its typed diagnostic"

[ -z "$(jobs -pr)" ] || fail "CORE-001B_cleanup leaked a child process"
if find "$HAPPY" "$INVALID" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then

    fail "CORE-001B_cleanup leaked a temporary file"
fi

echo "PASS: deterministic CI containment contract"
