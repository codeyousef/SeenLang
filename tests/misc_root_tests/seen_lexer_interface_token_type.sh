#!/usr/bin/env bash
# Source-only contract for bootstrap lexer-interface token type annotations.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
INTERFACE_SOURCE="$ROOT_DIR/compiler_seen/src/lexer/interfaces.seen"

fail() {
    echo "FAIL: lexer interface token type contract: $*" >&2
    exit 1
}

require_literal() {
    local literal=$1
    local label=$2
    rg -Fq -- "$literal" "$INTERFACE_SOURCE" || fail "$label"
}

[ -f "$INTERFACE_SOURCE" ] || fail "missing source: ${INTERFACE_SOURCE#"$ROOT_DIR"/}"
[ ! -L "$INTERFACE_SOURCE" ] || \
    fail "source must not be a symlink: ${INTERFACE_SOURCE#"$ROOT_DIR"/}"

require_literal 'import lexer.token_type.{SeenTokenType}' \
    'lexer interface must import SeenTokenType'
require_literal \
    'fun tokenTypeIsOperator(tokenType: SeenTokenType) r: Bool {' \
    'operator stub must use the declared SeenTokenType'
require_literal \
    'fun tokenTypeIsLiteral(tokenType: SeenTokenType) r: Bool {' \
    'literal stub must use the declared SeenTokenType'
require_literal \
    'fun tokenTypeIsKeyword(tokenType: SeenTokenType) r: Bool {' \
    'keyword stub must use the declared SeenTokenType'

if rg -n 'tokenTypeIs(Operator|Literal|Keyword)\(tokenType: TokenType\)' \
    "$INTERFACE_SOURCE"; then

    fail 'undeclared TokenType was reintroduced in a bootstrap stub signature'
fi

echo 'PASS: lexer interface token type source contract'
