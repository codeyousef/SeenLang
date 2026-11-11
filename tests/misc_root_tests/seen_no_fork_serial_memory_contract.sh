#!/usr/bin/env bash
# FEL-3110/FEL-3112: a serial (--no-fork) build of a large graph must not keep
# every module's AST in the compiler process. The runtime never collects ASTs,
# so Pass 1b and Pass 2 run each module in one isolated worker at a time, and
# generic discovery runs in a forked pre-scan so the parent reparses only the
# modules that use generic lowering. The parent's live bytes across generic
# preparation and IR generation must stay flat, generics spread across modules
# must still lower correctly, and the object manifest must be written.
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
    echo '    "src/generic_box.seen",'
    echo '    "src/generic_user.seen",'
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

# Generic class and function defined in one module and specialized from
# another: the pre-scan must keep both modules for the parent's discovery.
cat >"$PROJECT/src/generic_box.seen" <<'SEEN'
class GenericBox<T> {
    var value: T
    fun read() r: T { return this.value }
}

pub fun pickSecond<T>(first: T, second: T) r: T {
    return second
}
SEEN
cat >"$PROJECT/src/generic_user.seen" <<'SEEN'
import serial_fixture.generic_box.{GenericBox, pickSecond}

class Pair { var left: Int }

pub fun genericTotal() r: Int {
    let box = GenericBox<Int>{value: 40}
    let pair = GenericBox<Pair>{value: Pair{left: 2}}
    let total = box.read() + pair.read().left
    return pickSecond<Int>(0, total)
}
SEEN

{
    echo 'import serial_fixture.generic_user.{genericTotal}'
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
    echo '    if genericTotal() != 42 { return 3 }'
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

# Object-manifest mode leaves linking to the caller, so link a second serial
# build to prove generics spread across modules still lower correctly.
(
    cd "$PROJECT"
    bash "$ATTESTED_SEEN" "$COMPILER" compile tests/main.seen \
        "$WORK_DIR/program" --fast --no-cache --no-fork --frozen \
        >"$WORK_DIR/link.log" 2>&1
) || { tail -n 40 "$WORK_DIR/link.log" >&2; fail "linked serial compile failed"; }
program_status=0
"$WORK_DIR/program" || program_status=$?
[ "$program_status" -eq 0 ] ||
    fail "serial program exited $program_status (3 means generic lowering is wrong)"

discovery=$(grep -F '"phase":"generic aggregate discovery"' "$TRACE" | tail -n 1)
case "$discovery" in
    *mode=worker*) ;;
    *) fail "generic discovery did not use the forked pre-scan: $discovery" ;;
esac
reparsed=$(printf '%s\n' "$discovery" | sed -n 's/.*reparsed=\([0-9][0-9]*\).*/\1/p')
[ -n "$reparsed" ] && [ "$reparsed" -lt 16 ] ||
    fail "generic discovery reparsed ${reparsed:-?} modules; expected only generic users"

phase_used() {
    sed -n "s/.*\"phase\":\"$1\".*mem_used=\([0-9][0-9]*\).*/\1/p" "$TRACE" |
        tail -n 1
}
checked=$(phase_used 'lexical semantic check')
discovered=$(phase_used 'generic aggregate discovery')
before=$(phase_used 'generic aggregate preparation')
after=$(phase_used 'ir generation and object cache')
[ -n "$checked" ] && [ -n "$discovered" ] && [ -n "$before" ] &&
    [ -n "$after" ] || fail "build trace lacks phase memory"
generic_growth=$((discovered - checked))
# Reparsing every full body in the parent retained about 210 MiB here; the
# pre-scan leaves under 1 MiB. Declaration registration after discovery is
# needed by code generation and is not covered by this limit.
generic_limit=$((16 * 1024 * 1024))
[ "$generic_growth" -le "$generic_limit" ] ||
    fail "parent grew $generic_growth bytes in generic discovery (limit $generic_limit)"
growth=$((after - before))
# In-process generation retained every module AST: about 1.4 GiB of parent
# growth for this fixture. One worker per module leaves about 20 MiB.
limit=$((64 * 1024 * 1024))
[ "$growth" -le "$limit" ] ||
    fail "parent grew $growth bytes across IR generation (limit $limit)"

echo "PASS: FEL-3110/FEL-3112 no-fork serial build keeps parent memory flat (generic $generic_growth, IR $growth bytes over $MODULES modules)"
