#!/usr/bin/env bash
# Static/no-build contract for explicit `seen pkg prebuild` project operands.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
MAIN_SOURCE="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
CLI_ACCEPTANCE="$ROOT_DIR/tests/misc_root_tests/seen_cli_surface.sh"

fail() {
    echo "FAIL: pkg prebuild operand resolution: $*" >&2
    exit 1
}

require_literal() {
    local path=$1
    local literal=$2
    local label=$3
    rg -Fq -- "$literal" "$path" || fail "$label"
}

for path in "$MAIN_SOURCE" "$CLI_ACCEPTANCE"; do
    [ -f "$path" ] || fail "missing source: ${path#"$ROOT_DIR"/}"
    [ ! -L "$path" ] || fail "source must not be a symlink: ${path#"$ROOT_DIR"/}"
done

require_literal "$MAIN_SOURCE" \
    'fun pathFinalSegmentEquals(path: String, expected: String) r: Bool {' \
    'missing separator-aware final-segment helper'
require_literal "$MAIN_SOURCE" \
    'return pathByteIsSeparator(path.byteAt(segmentStart - 1))' \
    'final-segment helper does not accept both supported path separators'
require_literal "$MAIN_SOURCE" \
    'fun resolveProjectManifestPathFromCwd() r: String {' \
    'omitted-operand discovery is not separated from explicit resolution'
require_literal "$MAIN_SOURCE" \
    'pathFinalSegmentEquals(resolvedTarget, "Seen.toml")' \
    'explicit manifest detection is not separator-aware'
require_literal "$MAIN_SOURCE" \
    'var manifestPath = resolveProjectManifestPathFromCwd()' \
    'pkg prebuild does not use the dedicated omitted-operand resolver'
require_literal "$CLI_ACCEPTANCE" \
    'pkg prebuild explicit empty project' \
    'missing explicit-empty CLI regression'
require_literal "$CLI_ACCEPTANCE" \
    'pkg prebuild explicit directory without manifest' \
    'missing explicit non-project directory regression'
require_literal "$CLI_ACCEPTANCE" \
    'pkg prebuild explicit non-manifest file' \
    'missing explicit non-manifest file regression'

if rg -n -F 'resolvedTarget.endsWith("/Seen.toml")' "$MAIN_SOURCE"; then
    fail 'POSIX-only explicit manifest detection was reintroduced'
fi
if rg -n -F 'return findNearestSeenTomlForInput(resolvedTarget)' \
    "$MAIN_SOURCE"; then

    fail 'explicit file operands can still select an ancestor manifest'
fi
if rg -n -F 'return findNearestSeenTomlFromDir(resolvedTarget)' \
    "$MAIN_SOURCE"; then

    fail 'explicit directory operands can still select an ancestor manifest'
fi

echo 'PASS: pkg prebuild explicit operand resolution source contract'
