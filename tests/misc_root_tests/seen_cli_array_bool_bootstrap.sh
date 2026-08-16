#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
CLI_SOURCE="$ROOT_DIR/compiler_seen/src/tools/cli_args.seen"
MAIN_SOURCE="$ROOT_DIR/compiler_seen/src/main_compiler.seen"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

require_file() {
    local path=$1
    [ -f "$path" ] || fail "missing bootstrap source: $path"
    [ ! -L "$path" ] || fail "bootstrap source must not be a symlink: $path"
}

require_literal() {
    local path=$1
    local literal=$2
    local label=$3
    rg -Fq -- "$literal" "$path" || fail "$label"
}

require_file "$CLI_SOURCE"
require_file "$MAIN_SOURCE"

# Generic scalar Array payloads occupy an i64 slot in the bootstrap ABI. Keep
# this metadata explicitly integer-encoded until Array<Bool>.push widens i1.
require_literal "$CLI_SOURCE" 'var optionIsValue: Array<Int>' \
    'CLI option-kind metadata must use bootstrap-safe i64 array slots'
require_literal "$CLI_SOURCE" 'optionIsValue: Array<Int>()' \
    'CLI option-kind metadata initializer must match its Array<Int> field'
require_literal "$CLI_SOURCE" \
    'result.optionIsValue.push(if isValue { 1 } else { 0 })' \
    'CLI option-kind metadata must explicitly encode Bool as 1/0'
require_literal "$MAIN_SOURCE" \
    'if parsed.optionIsValue[optionIndex] == 0 {' \
    'CLI option-kind metadata consumer must explicitly decode the integer slot'

if rg -n -F 'var optionIsValue: Array<Bool>' "$CLI_SOURCE"; then
    fail 'Array<Bool> reintroduces the bootstrap i1-to-i64 push mismatch'
fi
if rg -n -F 'result.optionIsValue.push(isValue)' "$CLI_SOURCE"; then
    fail 'direct Bool push reintroduces the bootstrap i1-to-i64 mismatch'
fi
if rg -n -F 'not parsed.optionIsValue[optionIndex]' "$MAIN_SOURCE"; then
    fail 'CLI option-kind metadata must be decoded explicitly from its i64 slot'
fi

echo 'CLI bootstrap array-width static contract passed'
