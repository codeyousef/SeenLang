#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_compatibility_manifest.py"
PRODUCTION="$ROOT_DIR/releases/compatibility-manifest.json"
SCHEMA="$ROOT_DIR/schemas/compatibility-manifest.schema.json"
FIXTURES="$ROOT_DIR/tests/fixtures/core-002a"
SOURCE="$ROOT_DIR/compiler_seen/src/release/compatibility.seen"
COMPILER_ENTRY="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
COMPILER_MANIFEST="$ROOT_DIR/compiler_seen/Seen.toml"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/compatibility_manifest.seen"
STAGE1_ACCEPTANCE="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: compatibility manifest contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init compatibility-manifest-contract-tests) ||
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
python3 "$ROOT_DIR/tests/misc_root_tests/seen_compatibility_manifest_unit.py" \
    >/dev/null || fail "validator unit coverage"
python3 "$CHECKER" "$PRODUCTION" > "$TEST_ROOT/production.json" ||
    fail "production manifest"
python3 "$CHECKER" "$FIXTURES/happy/compatibility-manifest.json" \
    > "$TEST_ROOT/happy-a.json" || fail "CORE-002A_happy"
python3 "$CHECKER" "$FIXTURES/happy/compatibility-manifest.json" \
    > "$TEST_ROOT/happy-b.json" || fail "CORE-002A_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-002A_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$FIXTURES/happy/expected.json" ||
    fail "CORE-002A_happy bytes changed"

if python3 "$CHECKER" "$FIXTURES/invalid/compatibility-manifest.json" \
    > "$TEST_ROOT/invalid.json" 2> "$TEST_ROOT/invalid.err"; then

    fail "CORE-002A_invalid was accepted"
fi
grep -Fq 'core.002a.invalid' "$TEST_ROOT/invalid.err" ||
    fail "CORE-002A_invalid omitted its typed diagnostic"

if python3 "$CHECKER" "$FIXTURES/limit/compatibility-manifest.json" \
    --max-bytes 1 > /dev/null 2> "$TEST_ROOT/limit.err"; then

    fail "CORE-002A_limit was accepted"
fi
grep -Fq 'core.002a.limit' "$TEST_ROOT/limit.err" ||
    fail "CORE-002A_limit omitted its typed diagnostic"

set +e
SEEN_CORE_002A_TEST_HOOKS=1 python3 "$CHECKER" \
    "$FIXTURES/cancel/compatibility-manifest.json" --test-cancel-after-read \
    > "$TEST_ROOT/cancel.json" 2> "$TEST_ROOT/cancel.err"
cancel_status=$?
set -e
[ "$cancel_status" -eq 130 ] || fail "CORE-002A_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] || fail "CORE-002A_cancel emitted partial JSON"
grep -Fq 'core.002a.cancelled' "$TEST_ROOT/cancel.err" ||
    fail "CORE-002A_cancel omitted its typed diagnostic"

python3 "$CHECKER" "$FIXTURES/happy/compatibility-manifest.json" \
    --fuzz-seconds "${SEEN_CORE_002A_FUZZ_SECONDS:-1}" --seed 1101 \
    > "$TEST_ROOT/fuzz.json" 2> "$TEST_ROOT/fuzz.err" ||
    fail "seed-1101 parser fuzz"
cmp -s "$TEST_ROOT/fuzz.json" "$FIXTURES/happy/expected.json" ||
    fail "fuzz changed canonical output"

for symbol in AsciiString RetryClass RedactionClass SeenError OperationContext \
    CompatibilityManifest validateCompatibilityManifest; do

    grep -Fq "$symbol" "$SOURCE" || fail "native Seen model omitted $symbol"
done
for code in core.002a.invalid core.002a.limit core.002a.cancelled \
    core.002a.platform; do

    grep -Fq "$code" "$SOURCE" || fail "native Seen model omitted $code"
done
grep -Fq 'import release.compatibility.' "$COMPILER_ENTRY" ||
    fail "bootstrap entry does not seed the compatibility module"
grep -Fq 'modules.push("release.compatibility")' "$COMPILER_ENTRY" ||
    fail "bootstrap module list omits the compatibility module"
grep -Fq 'if segment == "release"' "$COMPILER_ENTRY" ||
    fail "compiler import roots omit release modules"
grep -Fq '"src/release"' "$COMPILER_MANIFEST" ||
    fail "compiler package manifest omits release modules"
grep -Fq 'validateCompatibilityManifest' "$NATIVE_TEST" ||
    fail "native compiling regression is missing"
grep -Fq 'compiler_seen/tests/compatibility_manifest.seen' \
    "$STAGE1_ACCEPTANCE" ||
    fail "native compiling regression is not in Stage-1 acceptance"

for identity in seen-layout-abi-v2 seen-object-cache-abi-v3 \
    seen-prebuilt-package-v2 seen-package-interface-v2 \
    seen-package-object-manifest-v2 runtime-v3 SEENPKG1; do

    grep -Fq "$identity" "$PRODUCTION" ||
        fail "production manifest omitted current identity $identity"
done

[ -z "$(jobs -pr)" ] || fail "CORE-002A_cleanup leaked a child process"
if find "$FIXTURES" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then

    fail "CORE-002A_cleanup leaked a temporary file"
fi

echo "PASS: deterministic compatibility manifest contract"
