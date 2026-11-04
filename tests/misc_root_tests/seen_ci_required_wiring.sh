#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
REQUIRED="$ROOT_DIR/scripts/ci_required.sh"

fail() {
    echo "FAIL: required CI admission: $*" >&2
    exit 1
}

[ -x "$REQUIRED" ] && [ ! -L "$REQUIRED" ] || fail "required gate is missing or not executable"

set +e
outside_error=$(env -u SEEN_CI_CONTAINMENT_IN_SCOPE "$REQUIRED" 2>&1 >/dev/null)
outside_status=$?
set -e
[ "$outside_status" -eq 126 ] ||
    fail "inner required gate did not fail closed outside containment"
case "$outside_error" in
    *core.001b.invalid*) ;;
    *) fail "outside-containment rejection omitted its typed diagnostic" ;;
esac

echo "PASS: required CI rejects execution outside containment"
