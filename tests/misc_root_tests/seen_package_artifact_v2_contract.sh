#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER_SOURCE="$ROOT_DIR/compiler_seen/src/main_compiler.seen"
SIDECAR_SOURCE="$ROOT_DIR/tools/seen-pkg/internal/commands/local.go"
SIDECAR_TEST="$ROOT_DIR/tools/seen-pkg/internal/commands/backend_test.go"

require_literal() {
    local file="$1"
    local literal="$2"
    if ! grep -F -- "$literal" "$file" >/dev/null; then
        echo "FAIL: missing '$literal' in ${file#"$ROOT_DIR"/}" >&2
        exit 1
    fi
}

for schema in \
    seen-prebuilt-package-v2 \
    seen-package-interface-v2 \
    seen-package-object-manifest-v2 \
    seen-layout-abi-v2 \
    seen-object-cache-abi-v3
do
    require_literal "$COMPILER_SOURCE" "$schema"
    require_literal "$SIDECAR_SOURCE" "$schema"
done

require_literal "$COMPILER_SOURCE" 'var signature = "cache-v5"'
require_literal "$COMPILER_SOURCE" 'source-object-v5|'
require_literal "$COMPILER_SOURCE" 'irCacheKey = hashString("v5:'
require_literal "$COMPILER_SOURCE" 'canonicalAnnotations(func.decorators)'
require_literal "$COMPILER_SOURCE" 'canonicalAnnotations(cls.decorators)'
require_literal "$COMPILER_SOURCE" 'canonicalParameter(cls.fields[fieldIndex])'
require_literal "$COMPILER_SOURCE" 'enumNode.variants[variantIndex]'
require_literal "$COMPILER_SOURCE" 'let discoveredModules = collectModulePaths(entryPath, true)'
require_literal "$COMPILER_SOURCE" 'let actualDigest = computeFileSha256(interfaceIndex)'
require_literal "$COMPILER_SOURCE" 'let expectedDeclarations = appendPackageInterfaceIndexDeclarations('
require_literal "$COMPILER_SOURCE" 'has declaration metadata that does not match its interface sources'
require_literal "$COMPILER_SOURCE" 'content = content + "language = "'
require_literal "$COMPILER_SOURCE" 'return writeTextAtomically(manifestPath, content)'
require_literal "$COMPILER_SOURCE" 'rebuild using Seen 0.11'

if grep -F 'queueArtifactPackageObjectSourceModules' "$COMPILER_SOURCE" >/dev/null; then
    echo "FAIL: legacy object-source interface fallback remains reachable" >&2
    exit 1
fi

require_literal "$SIDECAR_SOURCE" 'validateArtifactTableHeader(interfaceRaw, prebuiltInterfaceSchema)'
require_literal "$SIDECAR_SOURCE" 'validateArtifactTableHeader(objectRaw, prebuiltObjectManifestSchema)'
require_literal "$SIDECAR_SOURCE" 'has a declaration fingerprint mismatch'
require_literal "$SIDECAR_SOURCE" 'prebuiltArtifactRebuildGuidance = "rebuild using Seen 0.11"'
require_literal "$SIDECAR_TEST" 'TestProductionFetchRejectsV1ArtifactWithRebuildGuidance'
require_literal "$SIDECAR_TEST" 'strings.Contains(stderr, "rebuild using Seen 0.11")'

echo "PASS: prebuilt package artifact v2 source contract"
