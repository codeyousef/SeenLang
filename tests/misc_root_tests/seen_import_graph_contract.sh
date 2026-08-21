#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-003a"
CHECKER="$ROOT_DIR/scripts/check_import_graph.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_import_graph.py"
SOURCE="$ROOT_DIR/compiler_seen/src/imports/graph.seen"
DIAGNOSTICS="$ROOT_DIR/compiler_seen/src/release/diagnostic_schema.seen"
ENTRY="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
DECL_SCAN="$ROOT_DIR/compiler_seen/src/codegen/ir_decl_scan.seen"
DECL_ITEMS="$ROOT_DIR/compiler_seen/src/codegen/ir_decl_items.seen"
MODULE_EMIT="$ROOT_DIR/compiler_seen/src/codegen/ir_module_emit.seen"
TYPE_MAPPING="$ROOT_DIR/compiler_seen/src/codegen/ir_type_mapping.seen"
C_IMPORT_GEN="$ROOT_DIR/compiler_seen/src/tools/c_import_gen.seen"
MANIFEST="$ROOT_DIR/compiler_seen/Seen.toml"
NATIVE_TEST="$ROOT_DIR/compiler_seen/tests/release/import_graph.seen"
EXAMPLE="$ROOT_DIR/compiler_seen/examples/import_graph_resolution.seen"
STAGE1="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
COMPATIBILITY="$ROOT_DIR/releases/compatibility-manifest.json"
COMPATIBILITY_SCHEMA="$ROOT_DIR/schemas/compatibility-manifest.schema.json"
ARCHITECTURE="$ROOT_DIR/docs/compiler-architecture.md"
CHANGELOG="$ROOT_DIR/CHANGELOG.md"
LEDGER="$ROOT_DIR/docs/architecture/native-boundaries.json"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "FAIL: import graph contract: $*" >&2
    exit 1
}

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
test_scope=$(seen_artifact_scope_init import-graph-contract-tests) ||
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

python3 "$ROOT_DIR/tests/misc_root_tests/seen_import_graph_unit.py" >/dev/null ||
    fail "unit and branch matrix"
python3 "$CHECKER" "$FIXTURES/happy/graph.json" \
    >"$TEST_ROOT/happy-a.json" || fail "CORE-003A_happy"
python3 "$CHECKER" "$FIXTURES/happy/graph.json" \
    >"$TEST_ROOT/happy-b.json" || fail "CORE-003A_happy repeat"
cmp -s "$TEST_ROOT/happy-a.json" "$TEST_ROOT/happy-b.json" ||
    fail "CORE-003A_happy was nondeterministic"
cmp -s "$TEST_ROOT/happy-a.json" "$FIXTURES/happy/expected.json" ||
    fail "CORE-003A_happy bytes changed"

if python3 "$CHECKER" "$FIXTURES/invalid/graph.json" \
    >"$TEST_ROOT/invalid.json" 2>"$TEST_ROOT/invalid.err"; then

    fail "CORE-003A_invalid was accepted"
fi
grep -Fq 'core.003a.invalid' "$TEST_ROOT/invalid.err" ||
    fail "CORE-003A_invalid omitted its typed diagnostic"

if python3 "$CHECKER" "$FIXTURES/limit/graph.json" --max-modules 1 \
    >"$TEST_ROOT/limit.json" 2>"$TEST_ROOT/limit.err"; then

    fail "CORE-003A_limit was accepted"
fi
grep -Fq 'core.003a.limit' "$TEST_ROOT/limit.err" ||
    fail "CORE-003A_limit omitted its typed diagnostic"

cancel_status=0
SEEN_IMPORT_GRAPH_TEST_HOOKS=1 python3 "$CHECKER" \
    "$FIXTURES/cancel/graph.json" --test-cancel-after-read \
    >"$TEST_ROOT/cancel.json" 2>"$TEST_ROOT/cancel.err" || cancel_status=$?
[ "$cancel_status" -eq 130 ] ||
    fail "CORE-003A_cancel returned $cancel_status"
[ ! -s "$TEST_ROOT/cancel.json" ] ||
    fail "CORE-003A_cancel emitted partial output"
grep -Fq 'core.003a.cancelled' "$TEST_ROOT/cancel.err" ||
    fail "CORE-003A_cancel omitted its typed diagnostic"

python3 "$CHECKER" "$FIXTURES/happy/graph.json" \
    --fuzz-seconds "${SEEN_CORE_003A_FUZZ_SECONDS:-1}" --seed 1101 \
    >"$TEST_ROOT/fuzz.json" 2>"$TEST_ROOT/fuzz.err" ||
    fail "seed-1101 import-graph fuzz"
cmp -s "$TEST_ROOT/fuzz.json" "$FIXTURES/happy/expected.json" ||
    fail "import-graph fuzz changed canonical output"
grep -Fq 'seed=1101' "$TEST_ROOT/fuzz.err" ||
    fail "import-graph fuzz omitted its seed evidence"

for symbol in ModuleImportGraph ImportGraphResolution \
    resolveRecursiveImportGraph \
    resolveRecursiveImportGraphBootstrap renderImportGraphResolution; do

    grep -Fq "$symbol" "$SOURCE" || fail "native API omitted $symbol"
done
for code in core.003a.invalid core.003a.limit core.003a.cancelled \
    core.003a.platform core.003a.cycle; do

    grep -Fq "$code" "$SOURCE" || fail "native API omitted $code"
done
for field in code subsystem operation message causes nativeCode retry redaction; do
    grep -Fq "$field" "$DIAGNOSTICS" ||
        fail "diagnostic construction omitted $field"
done
[ "$(grep -Fc 'resolveRecursiveImportGraphBootstrap(' "$ENTRY")" -ge 3 ] ||
    fail "compile, check, and JIT do not share the graph resolver"
[ "$(grep -Fc 'if not moduleGraphContainsCompilerSources(' "$ENTRY")" -ge 3 ] ||
    fail "compiler-owned graphs do not retain their bounded-memory schedule"
bridge_body=$(sed -n '/fun resolveRecursiveImportGraphBootstrap/,/fun renderImportGraphResolution/p' \
    "$SOURCE")
printf '%s\n' "$bridge_body" | grep -Fq 'checked.errorStorage' ||
    fail "bootstrap bridge does not read fallible error storage"
printf '%s\n' "$bridge_body" | grep -Fq 'checked.resolutionStorage' ||
    fail "bootstrap bridge does not read fallible success storage"
if printf '%s\n' "$bridge_body" | grep -Eq \
    'checked\.(isErr|unwrap|unwrapErr)\('; then

    fail "bootstrap bridge depends on stale generic Result method bodies"
fi
grep -Fq 'compilerImportGraphIdentities(modulePaths)' "$ENTRY" ||
    fail "compiler does not derive logical graph identities"
grep -Fq 'compilerImportGraphIdentities(checkModulePaths)' "$ENTRY" ||
    fail "check path does not derive logical graph identities"
graph_builder=$(sed -n '/fun buildCompilerImportScanGraph/,/^}/p' "$ENTRY")
if printf '%s\n' "$graph_builder" | grep -Fq 'parseModule('; then
    fail "graph extraction retains a duplicate full-module AST pass"
fi
printf '%s\n' "$graph_builder" | grep -Fq 'appendSourceTextImportEdges' ||
    fail "graph extraction does not reuse canonical lightweight discovery"
semantic_validator=$(sed -n '/fun validateLexicalSemanticModulePaths/,/^}/p' \
    "$ENTRY")
printf '%s\n' "$semantic_validator" | grep -Fq \
    'modulePaths.length() > 48 and g_noFork == 0' ||
    fail "large semantic validation does not select isolated workers"
printf '%s\n' "$semantic_validator" | grep -Fq 'processFork()' ||
    fail "large semantic validation retains reparsed ASTs in the parent"
printf '%s\n' "$semantic_validator" | grep -Fq 'waitPid(semanticPid)' ||
    fail "large semantic validation does not serialize worker cleanup"
grep -Fq 'fun validateIsolatedLexicalSemanticGraph' "$ENTRY" ||
    fail "large semantic registry has no isolated coordinator"
compile_command=$(sed -n '/fun compileCommandWithAllOptions/,/fun buildCommandWithOptions/p' \
    "$ENTRY")
grep -Fq 'isolateCompileSemanticGate' <<<"$compile_command" ||
    fail "compile does not isolate its large semantic registry"
grep -Fq 'processFork()' <<<"$compile_command" ||
    fail "compile retains its large semantic registry into code generation"
grep -Fq 'waitPid(semanticCoordinatorPid)' <<<"$compile_command" ||
    fail "compile does not reclaim the isolated semantic registry"
grep -Fq 'compilerManifestModuleIdentity' "$ENTRY" ||
    fail "external packages do not receive manifest-stable graph identities"
grep -Fq 'extractTomlPackageIdentity(manifestContent)' "$ENTRY" ||
    fail "graph identities are not namespaced by canonical package identity"
if grep -Eq 'detectModuleImportCycle|printModuleImportCycleError|error\[E094\]' \
    "$ENTRY"; then

    fail "conflicting production cycle fallback remains"
fi
if grep -Fq 'import codegen.ir_decl_items' "$DECL_SCAN"; then
    fail "declaration scan reintroduced the compiler codegen import cycle"
fi
if grep -Fq 'import codegen.ir_decl_items' "$MODULE_EMIT"; then
    fail "module emission reintroduced the compiler codegen import cycle"
fi
if grep -Fq 'recordCrossModuleDeclareImpl(' "$DECL_ITEMS"; then
    fail "declaration registration rebuilds the indexed registry as O(n^2) pipe strings"
fi
grep -Fq 'state.xmDeclareNamesArr.push(symbolName)' "$DECL_ITEMS" ||
    fail "declaration registration does not retain canonical indexed names"
grep -Fq 'state.xmDeclareStrsArr.push(declStr)' "$DECL_ITEMS" ||
    fail "declaration registration does not retain canonical indexed declarations"
grep -Fq 'fun normalizeDeclaredTypeNodeImpl' "$TYPE_MAPPING" ||
    fail "acyclic shared declaration-type normalization is missing"
if grep -Eq '^import tools\.c_import_(frontend|emit)' "$C_IMPORT_GEN"; then
    fail "C-import model reintroduced a frontend/emitter import cycle"
fi
grep -Fq 'modules.push("imports.graph")' "$ENTRY" ||
    fail "future bootstrap module list omits the graph"
grep -Fq '"src/imports"' "$MANIFEST" ||
    fail "compiler manifest omits the import module"
grep -Fq 'compiler_seen/tests/release/import_graph.seen' "$STAGE1" ||
    fail "Stage-1 acceptance omits native import-graph regression"
grep -Fq 'compiler_seen/examples/import_graph_resolution.seen' "$STAGE1" ||
    fail "Stage-1 acceptance omits the compiling API example"
for case_name in CORE-003A_happy CORE-003A_invalid CORE-003A_limit \
    CORE-003A_cancel CORE-003A_cleanup; do

    grep -Fq "$case_name" "$NATIVE_TEST" ||
        fail "native executable regression omitted $case_name"
done

python3 "$BENCHMARK" "$FIXTURES/happy/graph.json" \
    "$FIXTURES/happy/benchmark.json" >"$TEST_ROOT/benchmark.log" ||
    fail "5-warmup/30-sample benchmark"
grep -Fq 'warmups=5 samples=30 status=pass' "$TEST_ROOT/benchmark.log" ||
    fail "benchmark omitted policy evidence"

python3 -c 'import json,sys; manifest=json.load(open(sys.argv[1], encoding="utf-8")); assert manifest["components"]["compiler"]["object_cache_abi"] == "seen-object-cache-abi-v3"' \
    "$COMPATIBILITY" || fail "compatibility manifest does not bind import ordering"
grep -Fq 'seen-import-graph-v1 canonical module ordering' \
    "$COMPATIBILITY_SCHEMA" ||
    fail "compatibility schema does not explain the ordering binding"
grep -Fq 'seen-import-graph-v1' "$SOURCE" ||
    fail "native source omits graph compatibility identity"
grep -Fq 'resolveRecursiveImportGraph' "$EXAMPLE" ||
    fail "API example does not call the public Result entry point"
grep -Fq '### Deterministic import graph' "$ARCHITECTURE" ||
    fail "compiler architecture omits CORE-003A"
grep -Fq 'native bounded recursive import-graph resolution' "$CHANGELOG" ||
    fail "changelog omits CORE-003A"

# CORE-003A adds no FFI: the production boundary ledger must remain valid and
# the native implementation must not introduce an extern declaration.
python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$LEDGER" >/dev/null ||
    fail "native-boundary ledger"
if grep -Eq '^[[:space:]]*extern[[:space:]]+fun' "$SOURCE" "$DIAGNOSTICS"; then
    fail "native graph implementation introduced an unreviewed FFI boundary"
fi

python3 -c 'import json,sys; expected=json.load(open(sys.argv[1], encoding="utf-8")); assert expected == {"children": 0, "descriptors": 0, "tasks": 0, "temporary_files": []}' \
    "$FIXTURES/cleanup/expected.json" || fail "cleanup expectation is invalid"
[ -z "$(jobs -pr)" ] || fail "CORE-003A_cleanup leaked a child"
if find "$FIXTURES" -type f \( -name '*.tmp' -o -name '.*.tmp' \) \
    -print -quit | grep -q .; then

    fail "CORE-003A_cleanup found a temporary artifact"
fi

echo "PASS: deterministic recursive import-graph contract"
