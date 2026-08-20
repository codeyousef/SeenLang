#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECKER="$ROOT_DIR/scripts/check_compatibility_manifest.py"
PRODUCTION="$ROOT_DIR/releases/compatibility-manifest.json"
FIXTURES="$ROOT_DIR/tests/fixtures/core-002b"
SOURCE="$ROOT_DIR/compiler_seen/src/release/compatibility.seen"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/compatibility_manifest.seen"
COMPILER_ENTRY="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
INSTALLER="$ROOT_DIR/scripts/build_installers.sh"
DEB_INSTALLER="$ROOT_DIR/installer/linux/build-deb.sh"
RPM_INSTALLER="$ROOT_DIR/installer/linux/build-rpm.sh"
APPIMAGE_INSTALLER="$ROOT_DIR/installer/linux/build-appimage.sh"
CI_REQUIRED="$ROOT_DIR/scripts/ci_required.sh"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: compatibility manifest runtime: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init compatibility-manifest-runtime-tests) ||
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

python3 "$CHECKER" "$FIXTURES/happy/compatibility-manifest.json" \
    >"$TEST_ROOT/happy-a.json" || fail "CORE-002B_happy"
python3 "$CHECKER" "$FIXTURES/happy/compatibility-manifest.json" \
    >"$TEST_ROOT/happy-b.json" || fail "CORE-002B_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-002B_happy bytes were nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$FIXTURES/happy/expected.json" ||
    fail "CORE-002B_happy bytes changed"

if python3 "$CHECKER" "$FIXTURES/invalid/compatibility-manifest.json" \
    >"$TEST_ROOT/invalid.json" 2>"$TEST_ROOT/invalid.err"; then

    fail "CORE-002B_invalid was accepted by the schema oracle"
fi
if python3 "$CHECKER" "$FIXTURES/limit/compatibility-manifest.json" \
    --max-bytes 1 >/dev/null 2>"$TEST_ROOT/limit.err"; then

    fail "CORE-002B_limit was accepted by the schema oracle"
fi

cancel_status=0
SEEN_CORE_002A_TEST_HOOKS=1 python3 "$CHECKER" \
    "$FIXTURES/cancel/compatibility-manifest.json" \
    --test-cancel-after-read >"$TEST_ROOT/cancel.json" \
    2>"$TEST_ROOT/cancel.err" || cancel_status=$?
[ "$cancel_status" -eq 130 ] ||
    fail "CORE-002B_cancel fixture returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] ||
    fail "CORE-002B_cancel fixture emitted partial output"

python3 "$CHECKER" "$PRODUCTION" >"$TEST_ROOT/production.json" ||
    fail "production compatibility manifest"
cmp -s "$TEST_ROOT/production.json" "$PRODUCTION" ||
    fail "production manifest is not canonical generator output"

for symbol in CompatibilityReleaseInputs generateCompatibilityManifest \
    renderCompatibilityManifest parseCompatibilityManifest \
    consumeCompatibilityManifest writeCompatibilityManifest \
    readAndConsumeCompatibilityManifest; do

    grep -Fq "$symbol" "$SOURCE" || fail "native Seen API omitted $symbol"
done
for code in core.002b.invalid core.002b.limit core.002b.cancelled \
    core.002b.platform core.002b.mismatch core.002b.io; do

    grep -Fq "$code" "$SOURCE" || fail "native Seen API omitted $code"
done
for case_name in CORE-002B_happy CORE-002B_invalid CORE-002B_limit \
    CORE-002B_cancel CORE-002B_cleanup; do

    grep -Fq "$case_name" "$NATIVE_TEST" ||
        fail "native executable regression omitted $case_name"
done

grep -Fq 'readCompatibilityRuntimeSelection' "$COMPILER_ENTRY" ||
    fail "runtime package-client bridge does not consume the manifest"
grep -Fq 'compatibility.packageClientVersion' "$COMPILER_ENTRY" ||
    fail "runtime package-client version is not manifest-derived"
grep -Fq 'compatibility.packageClientProtocol' "$COMPILER_ENTRY" ||
    fail "runtime package-client protocol is not manifest-derived"
grep -Fq 'cp "$COMPATIBILITY_MANIFEST" "$staging/compatibility-manifest.json"' \
    "$INSTALLER" || fail "installers do not ship the runtime manifest"
grep -Fq 'cp "$SOURCE_DIR/compatibility-manifest.json" "$package_dir/usr/bin/"' \
    "$DEB_INSTALLER" || fail "DEB does not install the manifest beside seen"
grep -Fq 'install -m 644 compatibility-manifest.json %{buildroot}%{_bindir}/compatibility-manifest.json' \
    "$RPM_INSTALLER" || fail "RPM does not install the manifest beside seen"
grep -Fq '%{_bindir}/compatibility-manifest.json' "$RPM_INSTALLER" ||
    fail "RPM file manifest omits the compatibility manifest"
grep -Fq 'cp "$SOURCE_DIR/compatibility-manifest.json" "$appdir/usr/bin/"' \
    "$APPIMAGE_INSTALLER" || fail "AppImage does not install the manifest beside seen"
source_root_bindings=$(grep -Ec 'SEEN_COMPILER_SOURCE_ROOT=.*REPO_ROOT' \
    "$ROOT_DIR/scripts/safe_rebuild.sh")
[ "$source_root_bindings" -ge 3 ] ||
    fail "temporary source-built compilers are not consistently bound to the source manifest"
grep -Fq 'seen_compatibility_manifest_runtime.sh' "$CI_REQUIRED" ||
    fail "required CI omits the runtime compatibility contract"

[ -z "$(jobs -pr)" ] || fail "CORE-002B_cleanup leaked a child process"
if find "$FIXTURES" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then

    fail "CORE-002B_cleanup leaked a temporary file"
fi
python3 -c 'import json,sys; expected=json.load(open(sys.argv[1], encoding="utf-8")); assert expected == {"children": 0, "descriptors": 0, "tasks": 0, "temporary_files": []}' \
    "$FIXTURES/cleanup/expected.json" ||
    fail "CORE-002B_cleanup expectation is invalid"

echo "PASS: deterministic generation and runtime compatibility contract"
