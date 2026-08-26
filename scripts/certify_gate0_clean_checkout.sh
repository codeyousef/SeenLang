#!/usr/bin/env bash
# Produce P0-GATE0-001 evidence from a clean checkout under required-CI containment.
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
ARTIFACT_HELPER="$ROOT/scripts/artifact_root.sh"
ARTIFACT_WRAPPER="$ROOT/scripts/run_with_project_artifacts.sh"
HARD_SCOPE="$ROOT/scripts/run_in_hard_memory_scope.sh"
CHECKER="$ROOT/scripts/check_gate0_certification.py"
FROZEN="$ROOT/bootstrap/stage1_frozen"
PACKAGE_CLIENT="$ROOT/compiler_seen/target/seen-pkg"
FROZEN_HASH="$ROOT/bootstrap/stage1_frozen.sha256"
COMPATIBILITY_HASH="$ROOT/bootstrap/stage1_frozen.compatibility-manifest.sha256"
FIXTURE="$ROOT/tests/fixtures/p0-gate0-001/happy/evidence.json"
PACKAGE_FIXTURE="$ROOT/tests/fixtures/external_package"
FUZZ_SECONDS="${SEEN_P0_GATE0_FUZZ_SECONDS:-60}"

fail() { echo "gate0-certification: p0.gate0.001.$1: $2" >&2; exit "${3:-1}"; }

[ "$(uname -s)" = Linux ] || fail platform "clean-checkout certification requires Linux"
[ "${SEEN_CI_CONTAINMENT_IN_SCOPE:-0}" = 1 ] || fail unverified "required-CI containment marker is missing" 126
[ "${SEEN_LOW_MEMORY:-0}" = 1 ] && [ "${SEEN_JOBS:-0}" = 1 ] &&
    [ "${SEEN_OPT_JOBS:-0}" = 1 ] || fail limit "serial low-memory settings are required" 126
[ -x "$HARD_SCOPE" ] && [ -x "$ARTIFACT_WRAPPER" ] &&
    [ -x "$FROZEN" ] && [ -x "$CHECKER" ] ||
    fail invalid "required certification entrypoint is missing"
case "${SEEN_MAIN_VMEM_KB:-}" in
    ''|*[!0-9]*) fail limit "positive compiler VMEM limit is required" 126 ;;
    *) [ "$SEEN_MAIN_VMEM_KB" -gt 0 ] ||
        fail limit "positive compiler VMEM limit is required" 126 ;;
esac
"$HARD_SCOPE" --label "Gate 0 certification read-back" --verify-only -- >/dev/null ||
    fail unverified "kernel containment read-back failed" 126

cd -- "$ROOT"
if [ -n "$(git status --porcelain=v1 --untracked-files=all)" ]; then
    fail invalid "certification requires a clean tracked and untracked checkout"
fi
python3 scripts/check_ci_workflows.py >/dev/null
sha256sum -c "$FROZEN_HASH" >/dev/null
sha256sum -c "$COMPATIBILITY_HASH" >/dev/null

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER"
seen_artifact_root_init "$ROOT" || fail invalid "artifact root validation failed"
SCOPE="$(seen_artifact_scope_init gate0-certification)" || fail invalid "artifact scope creation failed"
WORK="$(seen_artifact_mktemp_dir "$SCOPE" run)" || fail invalid "work directory creation failed"
EVIDENCE="$SCOPE/evidence.json"
cleanup() {
    local status=$?
    case "$WORK" in "$SCOPE"/run.*) rm -rf -- "$WORK" ;; *) return 1 ;; esac
    return "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

echo "Gate 0: building and verifying the compiler from the hash-pinned frozen seed"
SEEN_GO="${SEEN_GO:-$(command -v go || true)}" \
    SEEN_LOW_MEMORY=1 SEEN_JOBS=1 SEEN_OPT_JOBS=1 \
    "$ROOT/scripts/safe_rebuild.sh" --tier full

[ -x "$ROOT/compiler_seen/target/seen" ] || fail unverified "full rebuild did not install the compiler"
[ -x "$PACKAGE_CLIENT" ] || fail unverified "full rebuild did not install the package client"
if readelf -d "$ROOT/compiler_seen/target/seen" |
    grep -Eq 'Shared library: \[(libSDL3\.so|libvulkan\.so)'; then

    fail unverified "rebuilt compiler retained an unused optional GPU/window dependency"
fi

# The full rebuild validates this same canonical test during Stage-1 acceptance,
# then deliberately removes .seen_cache before installing the verified compiler.
# Run it once more against that installed compiler so the certification evidence
# hashes a durable report produced after the complete fixed-point build.
TEST_JSON="$ROOT/.seen_cache/test/P0-GATE0-001.json"
TEST_JUNIT="$ROOT/.seen_cache/test/P0-GATE0-001.xml"
TEST_LOG="$WORK/gate0-test.log"
rm -f -- "$TEST_JSON" "$TEST_JUNIT"
echo "Gate 0: running the canonical Seen test against the installed compiler"
if ! prlimit --as="$((SEEN_MAIN_VMEM_KB * 1024))" -- timeout 600 \
    "$ARTIFACT_WRAPPER" gate0-certification-test -- \
    "$ROOT/compiler_seen/target/seen" test "$ROOT" \
        --filter P0-GATE0-001 --profile ci --jobs 1 --timeout 10m \
        --report json:.seen_cache/test/P0-GATE0-001.json \
        --report junit:.seen_cache/test/P0-GATE0-001.xml \
        >"$TEST_LOG" 2>&1; then

    cat "$TEST_LOG" >&2
    fail unverified "canonical Seen test failed"
fi
grep -Fq 'test result: 1 passed; 0 failed' "$TEST_LOG" ||
    fail unverified "canonical Seen test summary is missing"
[ -f "$TEST_JSON" ] && [ -f "$TEST_JUNIT" ] ||
    fail unverified "canonical Seen test report is missing"
python3 "$ROOT/scripts/check_test_reporters.py" --format json \
    --validate "$TEST_JSON" >/dev/null || fail unverified "canonical JSON test report is invalid"
python3 "$ROOT/scripts/check_test_reporters.py" --format junit \
    --validate "$TEST_JUNIT" >/dev/null || fail unverified "canonical JUnit test report is invalid"

echo "Gate 0: packaging a source package twice and comparing canonical bytes"
prlimit --as=2147483648 -- timeout 300 "$PACKAGE_CLIENT" pack "$PACKAGE_FIXTURE" \
    --output "$WORK/package-a.seenpkg.tgz" --quiet
prlimit --as=2147483648 -- timeout 300 "$PACKAGE_CLIENT" pack "$PACKAGE_FIXTURE" \
    --output "$WORK/package-b.seenpkg.tgz" --quiet
cmp -s "$WORK/package-a.seenpkg.tgz" "$WORK/package-b.seenpkg.tgz" ||
    fail unverified "package bytes are not deterministic"

echo "Gate 0: fuzz-smoking the bounded evidence parser for ${FUZZ_SECONDS}s with seed 1101"
python3 "$CHECKER" --evidence "$FIXTURE" --fuzz-seconds "$FUZZ_SECONDS" \
    --seed 1101 >/dev/null 2>"$WORK/fuzz.log"
grep -Fq 'seed=1101' "$WORK/fuzz.log" || fail unverified "fuzz evidence is missing"

source_commit="$(git rev-parse HEAD)"
compiler_sha="$(sha256sum "$FROZEN" | awk '{print $1}')"
compatibility_sha="$(sha256sum "$ROOT/bootstrap/stage1_frozen.compatibility-manifest.json" | awk '{print $1}')"
build_sha="$(sha256sum "$ROOT/compiler_seen/target/seen" | awk '{print $1}')"
test_sha="$(sha256sum "$TEST_JSON" | awk '{print $1}')"
fuzz_sha="$(sha256sum "$WORK/fuzz.log" | awk '{print $1}')"
package_sha="$(sha256sum "$WORK/package-a.seenpkg.tgz" | awk '{print $1}')"
memory_max_bytes="$((SEEN_MEMORY_GUARD_RSS_KB * 1024))"

rm -f -- "$EVIDENCE"
python3 "$CHECKER" --output "$EVIDENCE" --source-commit "$source_commit" \
    --compiler-sha256 "$compiler_sha" --compatibility-sha256 "$compatibility_sha" \
    --build-sha256 "$build_sha" --test-sha256 "$test_sha" \
    --fuzz-sha256 "$fuzz_sha" --package-sha256 "$package_sha" \
    --memory-max-bytes "$memory_max_bytes" --pids-max "$SEEN_MEMORY_GUARD_TASKS_MAX" \
    --timeout-seconds 600 >/dev/null
python3 "$CHECKER" --evidence "$EVIDENCE" >/dev/null

if [ -n "$(git status --porcelain=v1 --untracked-files=all)" ]; then
    fail invalid "certification changed the checkout"
fi
if [ -n "$(jobs -pr)" ]; then fail unverified "certification leaked child jobs"; fi
echo "Gate 0 evidence: $EVIDENCE"
echo "PASS: P0-GATE0-001 clean-checkout certification"
