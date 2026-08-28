#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)
COMPILER=${COMPILER:-${SEEN_BIN:-$ROOT/compiler_seen/target/seen}}

fail() {
    echo "FAIL: FEL-1547/1548 project graph regression: $*" >&2
    exit 1
}

[ -x "$COMPILER" ] || fail "compiler is unavailable: $COMPILER"
[ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" = 1 ] ||
    fail "project artifact namespace is required"
[ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = 1 ] ||
    fail "verified aggregate hard-memory scope is required"
[ "${SEEN_JOBS:-0}" = 1 ] && [ "${SEEN_OPT_JOBS:-0}" = 1 ] ||
    fail "serial compiler workers are required"

WORK=$(mktemp -d "$SEEN_ARTIFACT_ROOT/fel1547-1548.XXXXXX")
PROJECT="$WORK/project"
LOG="$WORK/compile.log"
OUTPUT="$WORK/transitive_graph"
mkdir -p "$PROJECT/src/graph" "$PROJECT/tests"

{
    echo 'manifest-version = 1'
    echo
    echo '[project]'
    echo 'name = "recursive_fixture"'
    echo 'version = "0.1.0"'
    echo 'language = "en"'
    echo 'modules = ['
    for index in $(seq 0 22); do
        printf '    "src/graph/module_%02d.seen",\n' "$index"
    done
    echo ']'
    echo
    echo '[build]'
    echo 'targets = ["native"]'
    echo 'profile = "release"'
    echo
    echo '[dependencies]'
} >"$PROJECT/Seen.toml"

for index in $(seq 0 21); do
    next=$((index + 1))
    module=$(printf '%s/src/graph/module_%02d.seen' "$PROJECT" "$index")
    {
        printf 'import recursive_fixture.graph.module_%02d.{value%02d,\n' "$next" "$next"
        printf '    mirror%02d}\n\n' "$next"
        if [ "$index" -eq 0 ]; then
            echo 'pub class GraphEngine {'
            echo '    static fun open() r: Int {'
            printf '        return value%02d()\n' "$next"
            echo '    }'
            echo '}'
            echo
            echo 'pub fun openGraph() r: Int {'
            printf '    return value%02d()\n' "$next"
            echo '}'
            echo
        fi
        printf 'pub fun value%02d() r: Int {\n' "$index"
        printf '    return value%02d()\n' "$next"
        echo '}'
        echo
        printf 'pub fun mirror%02d() r: Int {\n' "$index"
        printf '    return mirror%02d()\n' "$next"
        echo '}'
    } >"$module"
done

{
    echo 'pub fun value22() r: Int { return 22 }'
    echo 'pub fun mirror22() r: Int { return 22 }'
} >"$PROJECT/src/graph/module_22.seen"

{
    echo 'import recursive_fixture.graph.module_00.{GraphEngine, openGraph}'
    echo
    echo 'fun main() r: Int {'
    echo '    if openGraph() != 22 { return 1 }'
    echo '    if GraphEngine.open() != 22 { return 2 }'
    echo '    return 0'
    echo '}'
} >"$PROJECT/tests/main.seen"

"$COMPILER" pkg fetch "$PROJECT" >/dev/null ||
    fail "canonical project lock generation failed"
[ -f "$PROJECT/Seen.lock" ] || fail "package fetch did not publish Seen.lock"

(
    cd "$PROJECT"
    SEEN_MEMORY_LIMIT_BYTES=7516192768 \
        timeout 900 "$COMPILER" compile tests/main.seen "$OUTPUT" \
        --release --lto=thin --target-cpu=x86-64 --no-cache \
        --jobs 1 --opt-jobs 1 --no-fork --frozen >"$LOG" 2>&1
) || {
    tail -n 300 "$LOG" >&2
    fail "24-module release/ThinLTO compile failed"
}

grep -Fq 'Found 24 modules' "$LOG" || {
    tail -n 120 "$LOG" >&2
    fail "recursive discovery did not close the 24-module graph"
}
grep -Fq 'Large module graph: deferring whole-project validation' "$LOG" ||
    fail "24-module graph retained the whole-project AST preflight"
if grep -Eq 'integer constant must have integer type|invalid LLVM|allocation failure' "$LOG"; then
    tail -n 300 "$LOG" >&2
    fail "compiler emitted invalid IR or exceeded the 7 GiB allocation budget"
fi
[ -x "$OUTPUT" ] || fail "compiler did not publish an executable"
"$OUTPUT" || fail "compiled transitive graph executable failed"

echo "PASS: FEL-1547 recursive project imports close deterministically"
echo "PASS: FEL-1548 24-module release/ThinLTO build stays within 7 GiB"
