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
IR_DIR="$WORK/ir"
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
        if [ "$index" -eq 0 ]; then
            echo '///'
            echo '/// Production-style engine documentation.'
            echo '///'
            echo '/// The final bare separator remains a line comment.'
            echo '/// Recursive imports below must remain visible.'
            echo '///'
            echo
            echo '///'
            echo 'import definitely_missing_package.ghost.{Ghost}'
            echo 'this is deliberately invalid block-comment content'
            echo '///'
            echo
            # Match FEL-1547's production shape: a documented engine module
            # fans out across stdlib and multiline project imports. The final
            # bare documentation separator previously hid this entire body.
            echo 'import collections.byte_buffer.{ByteBuffer}'
            echo 'import core.result.{Result}'
            echo 'import crypto.sha256.{sha256}'
            echo 'import formats.safetensors.{SafeTensorFile}'
            echo 'import math.math.{sqrt}'
            for dependency in $(seq 1 8); do
                printf 'import recursive_fixture.graph.module_%02d.{value%02d,\n' \
                    "$dependency" "$dependency"
                printf '    mirror%02d,\n' "$dependency"
                printf '    shadow%02d}\n' "$dependency"
            done
            echo
            echo 'pub class GraphEngine {'
            echo '    static fun open() r: Int {'
            echo '        return value01() + value02() + value03() + value04() +'
            echo '            value05() + value06() + value07() + value08()'
            echo '    }'
            echo '}'
            echo
            echo 'pub fun openGraph() r: Int {'
            echo '    return GraphEngine.open()'
            echo '}'
            echo
        else
            printf 'import recursive_fixture.graph.module_%02d.{value%02d,\n' "$next" "$next"
            printf '    mirror%02d}\n\n' "$next"
        fi
        printf 'pub fun value%02d() r: Int {\n' "$index"
        printf '    return value%02d()\n' "$next"
        echo '}'
        echo
        printf 'pub fun mirror%02d() r: Int {\n' "$index"
        printf '    return mirror%02d()\n' "$next"
        echo '}'
        echo
        printf 'pub fun shadow%02d() r: Int {\n' "$index"
        printf '    return shadow%02d()\n' "$next"
        echo '}'
    } >"$module"
done

{
    echo 'pub fun value22() r: Int { return 22 }'
    echo 'pub fun mirror22() r: Int { return 22 }'
    echo 'pub fun shadow22() r: Int { return 22 }'
} >"$PROJECT/src/graph/module_22.seen"

{
    echo 'import crypto.sha256.{sha256}'
    echo 'import io.file.{readTextChecked}'
    echo 'import json.parser.{destroyJsonParseResult}'
    echo 'import json.strict.{StrictJsonLimits}'
    echo 'import json.value.{JsonValue}'
    echo 'import math.math.{abs}'
    echo 'import recursive_fixture.graph.module_00.{GraphEngine, openGraph}'
    echo
    echo 'fun main() r: Int {'
    echo '    if openGraph() != 176 { return 1 }'
    echo '    if GraphEngine.open() != 176 { return 2 }'
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
        --jobs 1 --opt-jobs 1 --no-fork --frozen \
        --emit-module-ir-dir "$IR_DIR" >"$LOG" 2>&1
) || {
    tail -n 300 "$LOG" >&2
    fail "24-module release/ThinLTO compile failed"
}

module_count=$(sed -n 's/.*Found \([0-9][0-9]*\) modules.*/\1/p' "$LOG" | tail -n 1)
if [ -z "$module_count" ] || [ "$module_count" -lt 24 ]; then
    tail -n 120 "$LOG" >&2
    fail "recursive discovery did not close the project and stdlib graph"
fi
grep -Fq 'Large module graph: deferring whole-project validation' "$LOG" ||
    fail "large graph retained the whole-project AST preflight"
if grep -Eq 'integer constant must have integer type|invalid LLVM|allocation failure' "$LOG"; then
    tail -n 300 "$LOG" >&2
    fail "compiler emitted invalid IR or exceeded the 7 GiB allocation budget"
fi
[ -x "$OUTPUT" ] || fail "compiler did not publish an executable"
grep -REq '^define .*@GraphEngine_open\(' "$IR_DIR" || {
    find "$IR_DIR" -maxdepth 1 -type f -print >&2
    fail "engine module omitted the GraphEngine_open implementation"
}
"$OUTPUT" || fail "compiled transitive graph executable failed"

echo "PASS: FEL-1547 recursive project imports close deterministically"
echo "PASS: FEL-1548 large release/ThinLTO build stays within 7 GiB"
