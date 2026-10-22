#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-simd-benchmark-metadata
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/simd-benchmark-metadata.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=simd-benchmark-metadata
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
compile_case fast metadata "$ROOT_DIR/tests/inference_simd/dot_benchmark_metadata.seen"
rg -Fxq 'BENCH: selected_variant=1' "$TMP_DIR/metadata-fast.log"
[ "$(rg -c '^BENCH: candidate_ns=' "$TMP_DIR/metadata-fast.log")" = 30 ]
[ "$(rg -c '^BENCH: scalar_ns=' "$TMP_DIR/metadata-fast.log")" = 30 ]
echo 'PASS: paired benchmark retains candidate selection after scalar control and preserves thirty raw pairs'
