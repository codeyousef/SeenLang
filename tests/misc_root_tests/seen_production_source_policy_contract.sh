#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-003d"
CHECKER="$ROOT_DIR/scripts/check_production_source_policy.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_production_source_policy.py"
SOURCE="$ROOT_DIR/compiler_seen/src/imports/graph.seen"
DIAGNOSTICS="$ROOT_DIR/compiler_seen/src/release/diagnostic_schema.seen"
ENTRY="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
SAFE_REBUILD="$ROOT_DIR/scripts/safe_rebuild.sh"
OBSOLETE_REWRITER="$ROOT_DIR/scripts/rewrite_codegen_tmp.py"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/release/production_source_policy.seen"
EXAMPLE="$ROOT_DIR/compiler_seen/examples/production_source_policy.seen"
STAGE1="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
ARCHITECTURE="$ROOT_DIR/docs/compiler-architecture.md"
BOOTSTRAP_DOC="$ROOT_DIR/docs/bootstrap.md"
LIMITATIONS="$ROOT_DIR/docs/known-limitations.md"
CHANGELOG="$ROOT_DIR/CHANGELOG.md"
COMPATIBILITY_SCHEMA="$ROOT_DIR/schemas/compatibility-manifest.schema.json"
LEDGER="$ROOT_DIR/docs/architecture/native-boundaries.json"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: production source policy contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init production-source-policy-contract-tests) ||
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

python3 "$ROOT_DIR/tests/misc_root_tests/seen_production_source_policy_unit.py" \
    >/dev/null || fail "unit and branch matrix"
python3 "$CHECKER" "$FIXTURES/happy/policy.json" \
    >"$TEST_ROOT/happy-a.json" || fail "CORE-003D_happy"
python3 "$CHECKER" "$FIXTURES/happy/policy.json" \
    >"$TEST_ROOT/happy-b.json" || fail "CORE-003D_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-003D_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$FIXTURES/happy/expected.json" ||
    fail "CORE-003D_happy bytes changed"

if python3 "$CHECKER" "$FIXTURES/invalid/policy.json" \
    >"$TEST_ROOT/invalid.json" 2>"$TEST_ROOT/invalid.err"; then
    fail "CORE-003D_invalid was accepted"
fi
grep -Fq 'core.003d.invalid' "$TEST_ROOT/invalid.err" ||
    fail "CORE-003D_invalid omitted its typed diagnostic"

if python3 "$CHECKER" "$FIXTURES/limit/policy.json" \
    >"$TEST_ROOT/limit.json" 2>"$TEST_ROOT/limit.err"; then
    fail "CORE-003D_limit was accepted"
fi
grep -Fq 'core.003d.limit' "$TEST_ROOT/limit.err" ||
    fail "CORE-003D_limit omitted its typed diagnostic"

cancel_status=0
SEEN_PRODUCTION_SOURCE_TEST_HOOKS=1 python3 "$CHECKER" \
    "$FIXTURES/cancel/policy.json" --test-cancel-after-read \
    >"$TEST_ROOT/cancel.json" 2>"$TEST_ROOT/cancel.err" || cancel_status=$?
[ "$cancel_status" -eq 130 ] || fail "CORE-003D_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] || fail "CORE-003D_cancel emitted partial output"
grep -Fq 'core.003d.cancelled' "$TEST_ROOT/cancel.err" ||
    fail "CORE-003D_cancel omitted its typed diagnostic"

python3 "$CHECKER" "$FIXTURES/happy/policy.json" \
    --fuzz-seconds "${SEEN_CORE_003D_FUZZ_SECONDS:-1}" --seed 1101 \
    >"$TEST_ROOT/fuzz.json" 2>"$TEST_ROOT/fuzz.err" ||
    fail "seed-1101 production-source fuzz"
cmp -s "$TEST_ROOT/fuzz.json" "$FIXTURES/happy/expected.json" ||
    fail "fuzz changed canonical output"
grep -Fq 'seed=1101' "$TEST_ROOT/fuzz.err" || fail "fuzz omitted seed evidence"

for symbol in ProductionSourcePlan productionSourceSha256IsCanonical \
    validateUnmodifiedProductionSources renderProductionSourcePlan; do
    grep -Fq "$symbol" "$SOURCE" || fail "native API omitted $symbol"
done
for code in core.003d.invalid core.003d.limit core.003d.cancelled \
    core.003d.platform; do
    grep -Fq "$code" "$SOURCE" || fail "native API omitted $code"
done
grep -Fq 'subsystem: AsciiString{ value: "compiler.source" }' "$DIAGNOSTICS" ||
    fail "diagnostic subsystem is not stable"
for field in code subsystem operation message causes nativeCode retry redaction; do
    grep -Fq "$field" "$DIAGNOSTICS" || fail "diagnostic omitted $field"
done

# No source transformer may remain. The bootstrap view must preserve exact
# bytes, including the historical triple-slash blocks that were once stripped.
[ ! -e "$OBSOLETE_REWRITER" ] || fail "obsolete source rewriter still ships"
copy_body=$(sed -n '/^copy_bootstrap_seen_tree()/,/^}/p' "$SAFE_REBUILD")
printf '%s\n' "$copy_body" | grep -Fq 'cmp -s "$src_file" "$dst_file"' ||
    fail "bootstrap source view does not verify byte identity"
if printf '%s\n' "$copy_body" | grep -Eq 'awk|sed|python|\.seen\)'; then
    fail "bootstrap source view still contains a source transform"
fi
grep -Fq 'all Seen source bytes verified unchanged' "$SAFE_REBUILD" ||
    fail "bootstrap source view does not report its invariant"
grep -Fq 'core.003d.invalid: production source input is unavailable' "$ENTRY" ||
    fail "compiler source-read failure is not stable"
if sed -n '/^fun checkCommandWithProfile(/,/^}/p' "$ENTRY" | \
    grep -Fq 'runCommand("cat '; then
    fail "compiler check path retains an implicit shell-read fallback"
fi

RED=""
NC=""
eval "$copy_body"
mkdir -p "$TEST_ROOT/source/nested"
printf '%s\n' '///' 'preserve this body exactly' '///' \
    'fun main() r: Int { return 0 }' >"$TEST_ROOT/source/nested/main.seen"
copy_bootstrap_seen_tree "$TEST_ROOT/source" "$TEST_ROOT/view" ||
    fail "byte-identical source view copy"
cmp -s "$TEST_ROOT/source/nested/main.seen" \
    "$TEST_ROOT/view/nested/main.seen" ||
    fail "bootstrap source view changed triple-slash bytes"
mkdir -p "$TEST_ROOT/source-link"
printf '%s\n' 'fun main() r: Int { return 0 }' \
    >"$TEST_ROOT/source-link/target.seen"
ln -s target.seen "$TEST_ROOT/source-link/alias.seen"
copy_bootstrap_seen_tree "$TEST_ROOT/source-link" "$TEST_ROOT/link-view" ||
    fail "bootstrap source view rejected a contained source alias"
[ -f "$TEST_ROOT/link-view/alias.seen" ] &&
    [ ! -L "$TEST_ROOT/link-view/alias.seen" ] &&
    cmp -s "$TEST_ROOT/source-link/target.seen" \
        "$TEST_ROOT/link-view/alias.seen" ||
    fail "bootstrap source view did not materialize alias bytes"
mkdir -p "$TEST_ROOT/source-escape"
printf '%s\n' 'outside' >"$TEST_ROOT/outside.seen"
ln -s ../outside.seen "$TEST_ROOT/source-escape/escape.seen"
if copy_bootstrap_seen_tree "$TEST_ROOT/source-escape" \
    "$TEST_ROOT/escape-view" 2>"$TEST_ROOT/link.err"; then
    fail "bootstrap source view accepted an escaping symbolic link"
fi
grep -Fq 'link escapes its tree' "$TEST_ROOT/link.err" ||
    fail "escaping-link refusal omitted its diagnostic"
for tree in compiler_seen/src seen_std/src; do
    tree_slug=${tree//\//-}
    copy_bootstrap_seen_tree "$ROOT_DIR/$tree" "$TEST_ROOT/$tree_slug" ||
        fail "full byte-identical copy of $tree"
    source_count=$(find -L "$ROOT_DIR/$tree" -type f | wc -l)
    view_count=$(find "$TEST_ROOT/$tree_slug" -type f | wc -l)
    [ "$source_count" -eq "$view_count" ] ||
        fail "$tree source view omitted a regular file"
done

for case_name in CORE-003D_happy CORE-003D_invalid CORE-003D_limit \
    CORE-003D_cancel CORE-003D_cleanup; do
    grep -Fq "$case_name" "$NATIVE_TEST" ||
        fail "native executable regression omitted $case_name"
done
grep -Fq 'validateUnmodifiedProductionSources' "$EXAMPLE" ||
    fail "API example does not call the public Result entry point"
for fixture in compiler_seen/tests/release/production_source_policy.seen \
    compiler_seen/examples/production_source_policy.seen; do
    grep -Fq "$fixture" "$STAGE1" || fail "Stage-1 acceptance omits $fixture"
done

python3 "$BENCHMARK" "$FIXTURES/happy/policy.json" \
    "$FIXTURES/happy/benchmark.json" >"$TEST_ROOT/benchmark.log" ||
    fail "5-warmup/30-sample benchmark"
grep -Fq 'warmups=5 samples=30 status=pass' "$TEST_ROOT/benchmark.log" ||
    fail "benchmark omitted policy evidence"

grep -Fq 'seen-production-source-policy-v1' "$COMPATIBILITY_SCHEMA" ||
    fail "compatibility schema omits the production-source policy binding"
grep -Fq '### Unmodified production source' "$ARCHITECTURE" ||
    fail "compiler architecture omits CORE-003D"
grep -Fq 'byte-identical bootstrap source view' "$BOOTSTRAP_DOC" ||
    fail "bootstrap documentation does not forbid source rewriting"
grep -Fq 'production source rewriting' "$LIMITATIONS" ||
    fail "known limitations do not exclude production source rewriting"
grep -Fq 'unmodified production source' "$CHANGELOG" ||
    fail "changelog omits CORE-003D"

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$LEDGER" >/dev/null ||
    fail "native-boundary ledger"
if grep -Eq '^[[:space:]]*extern[[:space:]]+fun' "$SOURCE" "$DIAGNOSTICS"; then
    fail "native production-source policy introduced unreviewed FFI"
fi
python3 -c 'import json,sys; expected=json.load(open(sys.argv[1], encoding="utf-8")); assert expected == {"children": 0, "descriptors": 0, "tasks": 0, "temporary_files": []}' \
    "$FIXTURES/cleanup/expected.json" || fail "cleanup expectation is invalid"
[ -z "$(jobs -pr)" ] || fail "CORE-003D_cleanup leaked a child"
if find "$FIXTURES" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then
    fail "CORE-003D_cleanup found a temporary artifact"
fi

echo "PASS: unmodified production source policy contract"
