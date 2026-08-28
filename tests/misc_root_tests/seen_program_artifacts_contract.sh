#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
CHECKER="$ROOT/scripts/check_program_artifacts.py"
FIXTURES="$ROOT/tests/fixtures/core-004f"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/seen-core004f.XXXXXX")
trap 'rm -rf -- "$WORK"' EXIT INT TERM

fail() {
  echo "FAIL: CORE-004F contract: $*" >&2
  exit 1
}

python3 -m py_compile "$CHECKER" || fail checker_syntax
python3 -m json.tool "$ROOT/schemas/program-artifacts.schema.json" \
  > /dev/null || fail schema_syntax
python3 -m unittest tests.runner.test_program_artifacts_unit \
  > "$WORK/unit.out" || fail unit_contract

positive_cases=(
  two-root-default cold-warm-no-cache release-full-lto release-thin-lto
  object-manifest package-graph installed-compiler path-remap
)
for case_name in "${positive_cases[@]}"; do
  evidence="$FIXTURES/$case_name/evidence.json"
  output="$WORK/$case_name.json"
  python3 "$CHECKER" --evidence "$evidence" --output "$output" \
    || fail "$case_name"
  cmp -s "$evidence" "$output" || fail "$case_name-canonical"
done

negative_cases=(
  stale-cache:stale_cache input-change:input_change
  unsupported-combination:unsupported_combination
)
for entry in "${negative_cases[@]}"; do
  case_name=${entry%%:*}
  code=${entry#*:}
  if python3 "$CHECKER" --evidence "$FIXTURES/$case_name/evidence.json" \
    > /dev/null 2> "$WORK/$case_name.err"; then
    fail "$case_name-accepted"
  fi
  grep -Fq "core.004f.$code" "$WORK/$case_name.err" \
    || fail "$case_name-code"
done

set +e
python3 "$CHECKER" \
  --evidence "$FIXTURES/two-root-default/evidence.json" \
  --test-cancel-after-read > /dev/null 2> "$WORK/cancel.err"
cancel_status=$?
set -e
[[ "$cancel_status" -eq 130 ]] || fail cancellation_status
grep -Fq "core.004f.cancelled" "$WORK/cancel.err" \
  || fail cancellation_code

python3 "$CHECKER" \
  --evidence "$FIXTURES/two-root-default/evidence.json" \
  --fuzz-seconds 0.01 --seed 1101 > /dev/null 2> "$WORK/fuzz.err" \
  || fail bounded_fuzz
grep -Fq "seed=1101" "$WORK/fuzz.err" || fail fuzz_seed

[[ -z "$(find "$WORK" -maxdepth 1 -name '.program-artifacts-*' -print -quit)" ]] \
  || fail atomic_cleanup
[[ -f "$FIXTURES/corpus/single-file/main.seen" ]] || fail single_file_corpus
[[ -f "$FIXTURES/corpus/multi-module/Seen.toml" ]] || fail multi_module_corpus
[[ -f "$FIXTURES/corpus/locked-package/Seen.lock" ]] || fail locked_corpus
[[ -f "$FIXTURES/corpus/runtime-context/main.seen" ]] || fail runtime_corpus
if grep -Eq $'\t/' "$FIXTURES/object-manifest/objects.tsv" \
  "$FIXTURES/path-remap/objects.tsv"; then
  fail physical_path_leak
fi

NATIVE="$ROOT/compiler_seen/src/release/program_artifacts.seen"
NATIVE_TEST="$ROOT/compiler_seen/tests/reproducibility/core_004f_programs.seen"
COMPILER="$ROOT/compiler_seen/src/main_compiler.seen"
grep -Fq 'seen-program-artifacts-v1' "$NATIVE" || fail native_schema
for name in \
  CORE-004F_two_root_default CORE-004F_cold_warm_no_cache \
  CORE-004F_release_full_lto CORE-004F_release_thin_lto \
  CORE-004F_object_manifest CORE-004F_package_graph \
  CORE-004F_installed_compiler CORE-004F_path_remap \
  CORE-004F_stale_cache CORE-004F_input_change \
  CORE-004F_unsupported_combination; do
  grep -Fq "$name" "$NATIVE_TEST" || fail "$name-native"
done
for hook in \
  canonicalProgramBuildInputIdentity sourceObjectCacheIdentity \
  cachedObjectIsFresh storeCachedObjectAtomically \
  sortObjectInputsByStableSource copyProgramArtifactAtomically \
  seen-program-build-input-v1 seen-object-cache-record-v1 \
  seen-release-lto-cache-record-v1 outputTemporaryPath \
  --build-id=sha1 -no_uuid --no-insert-timestamp \
  'tar --sort=name' 'gzip -n -9'; do
  grep -Fq -- "$hook" "$COMPILER" || fail "compiler-hook-$hook"
done

echo "PASS: CORE-004F reproducible program artifacts contract"
