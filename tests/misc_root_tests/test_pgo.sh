#!/usr/bin/env bash
# Execute the full PGO cycle; missing profiles or tools are failures, not skips.
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT/compiler_seen/target/seen}"
CAPPED="$ROOT/scripts/run_capped_regression.sh"
SCOPE=pgo-cli
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT"
BASE_WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/pgo-cli.XXXXXX")"

compile() {
    local mode=$1
    shift
    timeout --foreground --kill-after=5s 600s \
        bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" compile \
        "$SOURCE" "$WORK/$mode" --release "--lto=$lto" --target-cpu=x86-64 \
        --no-cache --no-fork "$@" >"$WORK/$mode.compile.log" 2>&1
    if [ "$lto" = full ] && [ "$layout" = cross ]; then
        grep -Fq 'Release mode: cross-module LTO merge (2 modules)' \
            "$WORK/$mode.compile.log"
    fi
}
for layout in single cross; do
if [ "$layout" = single ]; then
    SOURCE="$ROOT/tests/fixtures/test-002a/pgo-single/main.seen"
else
    SOURCE="$ROOT/tests/fixtures/test-002a/pgo-cross/main.seen"
fi
for lto in thin full; do
WORK="$BASE_WORK/$layout-$lto"
mkdir -p -- "$WORK/profiles"
cd -- "$WORK"
if [ "$lto" = full ]; then
    compile unprofiled
    timeout --foreground --kill-after=5s 30s "$WORK/unprofiled" \
        >"$WORK/unprofiled.run.log" 2>&1
    grep -Fxq '499500' "$WORK/unprofiled.run.log"
fi
compile generate --pgo-generate
timeout --foreground --kill-after=5s 30s \
    env LLVM_PROFILE_FILE="$WORK/profiles/%m-%p.profraw" \
    "$WORK/generate" >"$WORK/generate.run.log" 2>&1
grep -Fxq '499500' "$WORK/generate.run.log"
shopt -s nullglob
profiles=("$WORK"/profiles/*.profraw)
[ "${#profiles[@]}" -gt 0 ] || {
    echo 'FAIL: PGO execution produced no raw profile' >&2
    exit 1
}
for profile in "${profiles[@]}"; do test -s "$profile"; done
timeout --foreground --kill-after=5s 60s llvm-profdata merge --num-threads=1 \
    -o "$WORK/merged.profdata" "${profiles[@]}" >"$WORK/merge.log" 2>&1
test -s "$WORK/merged.profdata"
timeout --foreground --kill-after=5s 60s llvm-profdata show \
    --all-functions --counts "$WORK/merged.profdata" >"$WORK/profile.log" 2>&1
grep -Eq 'Total functions: [1-9][0-9]*' "$WORK/profile.log"
grep -Eq '^  main:$' "$WORK/profile.log"
grep -Eq '^  compute:$' "$WORK/profile.log"
awk '
    /^  (main|compute):$/ { selected = 1 }
    selected && /Block counts:/ {
        if ($0 !~ /[1-9]/) exit 1
        measured++
        selected = 0
    }
    END { if (measured != 2) exit 1 }
' "$WORK/profile.log"
compile optimized --pgo-use=merged.profdata
timeout --foreground --kill-after=5s 30s "$WORK/optimized" \
    >"$WORK/optimized.run.log" 2>&1
grep -Fxq '499500' "$WORK/optimized.run.log"
cmp -- "$WORK/generate.run.log" "$WORK/optimized.run.log"
echo "PASS: PGO $layout $lto Seen counters, serial profile merge and profile-use execution"
done
done
