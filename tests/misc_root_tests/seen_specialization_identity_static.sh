#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
IDENTITY_SOURCE="$ROOT_DIR/compiler_seen/src/typechecker/specialization_identity.seen"
CACHE_SOURCE="$ROOT_DIR/compiler_seen/src/typechecker/specialization_cache.seen"
SEEN_FIXTURE="$ROOT_DIR/compiler_seen/tests/specialization_identity.seen"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

require_file() {
    local path=$1
    [ -f "$path" ] || fail "missing source-level contract file: $path"
    [ ! -L "$path" ] || fail "contract file must not be a symlink: $path"
}

require_literal() {
    local path=$1
    local literal=$2
    local label=$3
    rg -Fq -- "$literal" "$path" || fail "$label"
}

require_file "$IDENTITY_SOURCE"
require_file "$CACHE_SOURCE"
require_file "$SEEN_FIXTURE"

require_literal "$IDENTITY_SOURCE" \
    'import typechecker.type_ref.{TypeRef}' \
    'specialization identity must consume structured TypeRef data'
require_literal "$IDENTITY_SOURCE" \
    'import typechecker.semantic_registry.{SemanticDiagnostic}' \
    'specialization rejection must use the shared semantic diagnostic API'
require_literal "$IDENTITY_SOURCE" 'let canonicalDeclaration: String' \
    'key is missing canonical declaration identity'
require_literal "$IDENTITY_SOURCE" 'let definitionFingerprint: String' \
    'key is missing canonical definition fingerprint'
require_literal "$IDENTITY_SOURCE" \
    'let canonicalTypeArguments: Array<String>' \
    'key is missing canonical structured type arguments'
require_literal "$IDENTITY_SOURCE" \
    'let canonicalComptimeArguments: Array<String>' \
    'key is missing canonical typed comptime arguments'
require_literal "$IDENTITY_SOURCE" 'let canonicalTarget: String' \
    'key is missing canonical target identity'
require_literal "$IDENTITY_SOURCE" 'typeRef.canonicalIdentity()' \
    'TypeRef arguments are not snapshotted through canonicalIdentity()'
require_literal "$IDENTITY_SOURCE" 'seen-specialization-key-v1' \
    'specialization key serialization is not versioned'
require_literal "$IDENTITY_SOURCE" \
    'seen-comptime-argument-v1' \
    'typed comptime argument serialization is not versioned'
require_literal "$IDENTITY_SOURCE" \
    'specializationAppendComponent(out, "target", canonicalTarget)' \
    'target is absent from canonical key serialization'
require_literal "$IDENTITY_SOURCE" \
    'specializationAppendComponent(out, "definition",' \
    'definition fingerprint is absent from canonical key serialization'

require_literal "$IDENTITY_SOURCE" \
    'fun buildSpecializationDemand(' \
    'explicit specialization demand diagnostic boundary is missing'
require_literal "$IDENTITY_SOURCE" 'typeRef.isOpen(genericNames)' \
    'declared source generics are not detected'
require_literal "$IDENTITY_SOURCE" \
    'typeRef.identityBase.contains("generic:")' \
    'semantic generic identity markers are not detected'
require_literal "$IDENTITY_SOURCE" \
    'E_SPECIALIZATION_OPEN_GENERIC' \
    'open specialization demand diagnostic is missing'
require_literal "$IDENTITY_SOURCE" \
    'E_SPECIALIZATION_UNRESOLVED_TYPE' \
    'unresolved specialization type diagnostic is missing'
require_literal "$IDENTITY_SOURCE" 'E_SPECIALIZATION_COMPTIME' \
    'invalid comptime demand diagnostic is missing'

require_literal "$IDENTITY_SOURCE" 'return 65536' \
    'hard specialization ceiling is not 65,536'
require_literal "$CACHE_SOURCE" \
    'this.orderedKeys.length() >= specializationLimit()' \
    'cache does not enforce the hard specialization ceiling'
require_literal "$CACHE_SOURCE" 'E_SPECIALIZATION_LIMIT' \
    'specialization ceiling has no explicit diagnostic'
require_literal "$CACHE_SOURCE" 'fun lowerBound(' \
    'cache has no deterministic sorted insertion boundary'
require_literal "$CACHE_SOURCE" \
    'specializationCompareCanonical(' \
    'cache ordering does not use locale-independent canonical comparison'
require_literal "$CACHE_SOURCE" \
    'this.orderedKeys[shift] = this.orderedKeys[shift - 1]' \
    'cache insertion does not preserve canonical ordering'
require_literal "$CACHE_SOURCE" 'fun canonicalIndex()' \
    'cache lacks deterministic ordered serialization'

existing_line="$(rg -n -F 'let existing = this.findCanonical' \
    "$CACHE_SOURCE" | head -n 1)"
limit_line="$(rg -n -F \
    'this.orderedKeys.length() >= specializationLimit()' \
    "$CACHE_SOURCE" | head -n 1)"
[ -n "$existing_line" ] && [ -n "$limit_line" ] || \
    fail 'could not locate duplicate-before-limit cache policy'
existing_line=${existing_line%%:*}
limit_line=${limit_line%%:*}
[ "$existing_line" -lt "$limit_line" ] || \
    fail 'duplicate cache hits must be checked before the hard ceiling'

require_literal "$IDENTITY_SOURCE" '__seen_spec_v1_' \
    'deterministic internal mangle is not versioned'
require_literal "$IDENTITY_SOURCE" 'specializationHexEncode(canonicalKey)' \
    'internal mangle is not collision-free over the full canonical key'
if rg -n '^[[:space:]]*import[[:space:]]+(codegen|llvm|ir\.)' \
    "$IDENTITY_SOURCE" "$CACHE_SOURCE"; then

    fail 'ABI-neutral specialization foundation imported a lowering layer'
fi

require_literal "$SEEN_FIXTURE" \
    'import typechecker.specialization_identity.' \
    'Seen fixture does not import specialization identity API'
require_literal "$SEEN_FIXTURE" \
    'import typechecker.specialization_cache.{SpecializationCache}' \
    'Seen fixture does not import specialization cache API'
require_literal "$SEEN_FIXTURE" \
    '"target participates in identity"' \
    'Seen fixture omits target identity coverage'
require_literal "$SEEN_FIXTURE" \
    '"definition fingerprint participates in identity"' \
    'Seen fixture omits definition fingerprint coverage'
require_literal "$SEEN_FIXTURE" \
    '"source generic is rejected explicitly"' \
    'Seen fixture omits open-generic rejection coverage'
require_literal "$SEEN_FIXTURE" \
    '"stable cache index is insertion-order independent"' \
    'Seen fixture omits deterministic cache ordering coverage'
require_literal "$SEEN_FIXTURE" \
    'specializationLimit() == 65536' \
    'Seen fixture omits hard specialization ceiling contract'

echo 'specialization identity/cache static contract passed'
