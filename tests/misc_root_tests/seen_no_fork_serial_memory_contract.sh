#!/usr/bin/env bash
# FEL-3110: a serial (--no-fork) build of a large graph must not keep every
# module's AST in the compiler process. The runtime never collects ASTs, so
# Pass 1b and Pass 2 run each module in one isolated worker at a time; the
# parent's live bytes across IR generation must stay flat, and the object
# manifest must be written.
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-no-fork-serial-memory
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/no-fork-serial-memory.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/no-fork-serial-memory.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved no-fork serial memory artifacts: $WORK_DIR" >&2
            fi
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT

fail() {
    echo "FAIL: FEL-3110 no-fork serial memory: $*" >&2
    exit 1
}

MODULES=64
PROJECT="$WORK_DIR/project"
mkdir -p "$PROJECT/src/parts" "$PROJECT/tests"
{
    echo 'manifest-version = 1'
    echo
    echo '[project]'
    echo 'name = "serial_fixture"'
    echo 'version = "0.1.0"'
    echo 'language = "en"'
    echo 'modules = ['
    for index in $(seq 0 $((MODULES - 1))); do
        printf '    "src/parts/part_%02d.seen",\n' "$index"
    done
    echo ']'
    echo
    echo '[build]'
    echo 'targets = ["native"]'
    echo
    echo '[dependencies]'
} >"$PROJECT/Seen.toml"

# Enough body per module that a retained AST is visible in the live bytes.
for index in $(seq 0 $((MODULES - 1))); do
    {
        for fn in $(seq 0 39); do
            printf 'fun part%02dStep%02d(value: Int) r: Int {\n' "$index" "$fn"
            echo '    var total = value'
            echo '    var step = 0'
            echo '    while step < 8 {'
            echo '        if total % 3 == 0 { total = total + step * 2 }'
            echo '        else if total % 3 == 1 { total = total - step }'
            echo '        else { total = total + 1 }'
            echo '        step = step + 1'
            echo '    }'
            echo '    return total'
            echo '}'
            echo
        done
        printf 'pub fun part%02d() r: Int {\n' "$index"
        printf '    return part%02dStep00(%d) %% 7\n' "$index" "$index"
        echo '}'
    } >"$PROJECT/src/parts/part_$(printf '%02d' "$index").seen"
done

{
    for index in $(seq 0 $((MODULES - 1))); do
        printf 'import serial_fixture.parts.part_%02d.{part%02d}\n' \
            "$index" "$index"
    done
    echo
    echo 'fun main() r: Int {'
    echo '    var total = 0'
    for index in $(seq 0 $((MODULES - 1))); do
        printf '    total = total + part%02d()\n' "$index"
    done
    echo '    if total < 0 { return 1 }'
    echo '    return 0'
    echo '}'
} >"$PROJECT/tests/main.seen"

"$COMPILER" pkg fetch "$PROJECT" >/dev/null || fail "project lock generation failed"

TRACE="$WORK_DIR/trace.jsonl"
LOG="$WORK_DIR/compile.log"
MANIFEST="$WORK_DIR/objects.tsv"
(
    cd "$PROJECT"
    SEEN_TRACE_BUILD="$TRACE" bash "$ATTESTED_SEEN" "$COMPILER" compile \
        tests/main.seen "$WORK_DIR/out" --fast --no-cache --no-fork \
        --frozen --object-manifest "$MANIFEST" \
        >"$LOG" 2>&1
) || { tail -n 80 "$LOG" >&2; fail "serial compile failed"; }

grep -Fq 'Large project worker pool: IR 1 jobs' "$LOG" ||
    fail "--no-fork large graph did not use one isolated IR worker"
[ -s "$MANIFEST" ] || fail "object manifest was not written"
manifest_parts=$(grep -c '/src/parts/part_' "$MANIFEST" || true)
[ "$manifest_parts" -eq "$MODULES" ] ||
    fail "object manifest lists $manifest_parts of $MODULES project modules"

phase_used() {
    sed -n "s/.*\"phase\":\"$1\".*mem_used=\([0-9][0-9]*\).*/\1/p" "$TRACE" |
        tail -n 1
}
before=$(phase_used 'generic aggregate preparation')
after=$(phase_used 'ir generation and object cache')
[ -n "$before" ] && [ -n "$after" ] || fail "build trace lacks phase memory"
growth=$((after - before))
# In-process generation retained every module AST: about 1.4 GiB of parent
# growth for this fixture. One worker per module leaves about 20 MiB.
limit=$((64 * 1024 * 1024))
[ "$growth" -le "$limit" ] ||
    fail "parent grew $growth bytes across IR generation (limit $limit)"

echo "PASS: FEL-3110 no-fork serial build keeps parent memory flat ($growth bytes over $MODULES modules)"
