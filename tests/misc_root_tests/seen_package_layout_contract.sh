#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/pkg-layout-001"
EXTERNAL="$ROOT_DIR/tests/fixtures/external_package"
EXTERNAL_CONSUMER="$FIXTURES/external-consumer"
SOURCE="$ROOT_DIR/compiler_seen/src/release/compatibility.seen"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/package_layout.seen"
STAGE1="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
PRODUCTION="$ROOT_DIR/releases/compatibility-manifest.json"
COMPATIBILITY_CHECKER="$ROOT_DIR/scripts/check_compatibility_manifest.py"
LAYOUT_CHECKER="$ROOT_DIR/scripts/check_package_layout.py"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: package layout contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init package-layout-contract-tests) ||
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

python3 "$ROOT_DIR/tests/misc_root_tests/seen_package_layout_unit.py" ||
    fail "package-layout unit coverage"
python3 "$LAYOUT_CHECKER" "$FIXTURES/happy/layout.json" \
    >"$TEST_ROOT/happy-a.json" || fail "PKG-LAYOUT-001_happy"
python3 "$LAYOUT_CHECKER" "$FIXTURES/happy/layout.json" \
    >"$TEST_ROOT/happy-b.json" || fail "PKG-LAYOUT-001_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "PKG-LAYOUT-001_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$FIXTURES/happy/expected.json" ||
    fail "PKG-LAYOUT-001_happy bytes changed"
if python3 "$LAYOUT_CHECKER" "$FIXTURES/invalid/layout.json" \
    >"$TEST_ROOT/invalid.json" 2>"$TEST_ROOT/invalid.err"; then

    fail "PKG-LAYOUT-001_invalid was accepted"
fi
grep -Fq 'pkg.layout.001.invalid' "$TEST_ROOT/invalid.err" ||
    fail "PKG-LAYOUT-001_invalid omitted its typed diagnostic"
if python3 "$LAYOUT_CHECKER" "$FIXTURES/limit/layout.json" --max-bytes 1 \
    >"$TEST_ROOT/limit.json" 2>"$TEST_ROOT/limit.err"; then

    fail "PKG-LAYOUT-001_limit was accepted"
fi
grep -Fq 'pkg.layout.001.limit' "$TEST_ROOT/limit.err" ||
    fail "PKG-LAYOUT-001_limit omitted its typed diagnostic"
cancel_status=0
SEEN_PKG_LAYOUT_TEST_HOOKS=1 python3 "$LAYOUT_CHECKER" \
    "$FIXTURES/cancel/layout.json" --test-cancel-after-read \
    >"$TEST_ROOT/cancel.json" 2>"$TEST_ROOT/cancel.err" || cancel_status=$?
[ "$cancel_status" -eq 130 ] ||
    fail "PKG-LAYOUT-001_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] ||
    fail "PKG-LAYOUT-001_cancel emitted partial output"
python3 "$LAYOUT_CHECKER" "$FIXTURES/happy/layout.json" \
    --fuzz-seconds "${SEEN_PKG_LAYOUT_FUZZ_SECONDS:-1}" --seed 1101 \
    >"$TEST_ROOT/fuzz.json" 2>"$TEST_ROOT/fuzz.err" ||
    fail "seed-1101 package-layout fuzz"
cmp -s "$TEST_ROOT/fuzz.json" "$FIXTURES/happy/expected.json" ||
    fail "package-layout fuzz changed canonical output"

for code in pkg.layout.001.invalid pkg.layout.001.limit \
    pkg.layout.001.cancelled pkg.layout.001.platform; do

    grep -Fq "$code" "$SOURCE" || fail "native API omitted $code"
done
for symbol in ReusablePackagePlatforms ReusablePackageLayout \
    validateReusablePackageLayout renderReusablePackageLayout; do

    grep -Fq "$symbol" "$SOURCE" || fail "native API omitted $symbol"
done
for case_name in PKG-LAYOUT-001_happy PKG-LAYOUT-001_invalid \
    PKG-LAYOUT-001_limit PKG-LAYOUT-001_cancel PKG-LAYOUT-001_cleanup; do

    grep -Fq "$case_name" "$NATIVE_TEST" ||
        fail "native executable regression omitted $case_name"
done
grep -Fq 'compiler_seen/tests/package_layout.seen' "$STAGE1" ||
    fail "Stage-1 acceptance omits package-layout regression"
grep -Fq 'stage_external_package_fixture' "$STAGE1" ||
    fail "Stage-1 acceptance does not isolate the external package fixture"

for relative in Seen.toml Seen.lock src/mod.seen tests/layout_contract.seen \
    examples/consumer.seen README.md LICENSE; do

    [ -f "$EXTERNAL/$relative" ] && [ ! -L "$EXTERNAL/$relative" ] ||
        fail "external fixture omitted canonical member $relative"
done
for relative in Seen.toml Seen.lock src/main.seen; do
    [ -f "$EXTERNAL_CONSUMER/$relative" ] && \
        [ ! -L "$EXTERNAL_CONSUMER/$relative" ] ||
        fail "external consumer omitted canonical member $relative"
done
grep -Fq 'modules = ["src/mod.seen"]' "$EXTERNAL/Seen.toml" ||
    fail "external package does not declare the canonical library entry"
grep -Fq 'identity = "seen/external-package"' "$EXTERNAL/Seen.toml" ||
    fail "external package identity is not canonical"
grep -Fq 'pub class PackageValue' "$EXTERNAL/src/mod.seen" ||
    fail "external package type is not public"
grep -Fq 'pub fun packageValue()' "$EXTERNAL/src/mod.seen" ||
    fail "external package function is not public"
grep -Fq 'fixture = { path = "../../external_package" }' \
    "$EXTERNAL_CONSUMER/Seen.toml" ||
    fail "external consumer does not use the package path boundary"
manifest_hash=$(sha256sum "$EXTERNAL/Seen.toml" | awk '{print $1}')
grep -Fq "manifest_sha256 = \"$manifest_hash\"" "$EXTERNAL/Seen.lock" ||
    fail "external package lock is stale"
consumer_hash=$(sha256sum "$EXTERNAL_CONSUMER/Seen.toml" | awk '{print $1}')
grep -Fq "manifest_sha256 = \"$consumer_hash\"" \
    "$EXTERNAL_CONSUMER/Seen.lock" || fail "external consumer lock is stale"

python3 "$COMPATIBILITY_CHECKER" "$PRODUCTION" >/dev/null ||
    fail "production compatibility manifest"
python3 -c 'import json,sys; manifest=json.load(open(sys.argv[1], encoding="utf-8")); assert manifest["components"]["compiler"]["package_interface_schema"] == "seen-package-interface-v2"' \
    "$PRODUCTION" || fail "compatibility manifest does not bind package interfaces"
grep -Fq '"seen-package-layout-v1"' "$SOURCE" ||
    fail "native compatibility module omits the package-layout identity"

python3 -c 'import json,sys; expected=json.load(open(sys.argv[1], encoding="utf-8")); assert expected == {"children": 0, "descriptors": 0, "tasks": 0, "temporary_files": []}' \
    "$FIXTURES/cleanup/expected.json" || fail "cleanup expectation is invalid"
[ -z "$(jobs -pr)" ] || fail "PKG-LAYOUT-001_cleanup leaked a child"
if find "$FIXTURES" "$EXTERNAL" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then

    fail "PKG-LAYOUT-001_cleanup found a temporary artifact"
fi
if find "$FIXTURES" "$EXTERNAL" -type d -name .seen -print -quit | grep -q .; then
    fail "PKG-LAYOUT-001_cleanup found generated package state"
fi

echo "PASS: canonical reusable-package layout contract"
