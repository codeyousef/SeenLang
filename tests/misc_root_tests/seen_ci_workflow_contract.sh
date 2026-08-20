#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_ci_workflows.py"
HAPPY="$ROOT_DIR/tests/fixtures/core-001a/happy"
INVALID="$ROOT_DIR/tests/fixtures/core-001a/invalid"
EXPECTED="$HAPPY/expected.json"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: CI workflow contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init ci-workflow-contract-tests) ||
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

python3 "$CHECKER" --root "$ROOT_DIR" > "$TEST_ROOT/production.json" ||
    fail "production CI contract"
python3 "$CHECKER" --root "$HAPPY" > "$TEST_ROOT/happy-a.json" ||
    fail "CORE-001A_happy"
python3 "$CHECKER" --root "$HAPPY" > "$TEST_ROOT/happy-b.json" ||
    fail "CORE-001A_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-001A_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$EXPECTED" ||
    fail "CORE-001A_happy bytes changed"

before_invalid=$(find "$INVALID" -type f -exec sha256sum {} + | sort)
if python3 "$CHECKER" --root "$INVALID" \
    > "$TEST_ROOT/invalid.json" 2> "$TEST_ROOT/invalid.err"; then
    fail "CORE-001A_invalid was accepted"
fi
grep -Fq 'core.001a.invalid' "$TEST_ROOT/invalid.err" ||
    fail "CORE-001A_invalid omitted its typed diagnostic"
after_invalid=$(find "$INVALID" -type f -exec sha256sum {} + | sort)
[ "$before_invalid" = "$after_invalid" ] || fail "CORE-001A_invalid mutated its input"

if python3 "$CHECKER" --root "$HAPPY" --max-files 1 >/dev/null 2>&1; then
    fail "CORE-001A_limit was accepted"
fi

mkdir -p "$TEST_ROOT/symlink/.github"
ln -s "$HAPPY/.github/workflows" "$TEST_ROOT/symlink/.github/workflows"
if python3 "$CHECKER" --root "$TEST_ROOT/symlink" >/dev/null 2>&1; then
    fail "CORE-001A_invalid accepted a workflow symlink"
fi

set +e
SEEN_CI_WORKFLOW_TEST_HOOKS=1 python3 "$CHECKER" --root "$HAPPY" \
    --test-cancel-after-files 1 > "$TEST_ROOT/cancel.json" 2> "$TEST_ROOT/cancel.err"
cancel_status=$?
set -e
[ "$cancel_status" -eq 130 ] || fail "CORE-001A_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] || fail "CORE-001A_cancel emitted partial JSON"
grep -Fq 'core.001a.cancelled' "$TEST_ROOT/cancel.err" ||
    fail "CORE-001A_cancel omitted its typed diagnostic"

[ -z "$(jobs -pr)" ] || fail "CORE-001A_cleanup leaked a child process"
if find "$HAPPY" "$INVALID" -type f \( -name '*.tmp' -o -name '.*.tmp' \) -print -quit |
    grep -q .; then
    fail "CORE-001A_cleanup leaked a temporary file"
fi

echo "PASS: deterministic CI workflow contract"
