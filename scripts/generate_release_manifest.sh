#!/usr/bin/env bash
# Generates a JSON release manifest for signed artifacts.
#
# Usage:
#   generate_release_manifest.sh <artifact-dir> > manifest.json
#
# The manifest includes checksums, sizes, and references to signature bundles.
# Output is written to stdout as JSON.
#
# Example:
#   ./scripts/generate_release_manifest.sh release-assets > release-assets/manifest.json

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <artifact-dir>" >&2
    exit 1
fi

ARTIFACT_DIR="$1"

if [[ ! -d "$ARTIFACT_DIR" ]]; then
    echo "Error: Directory not found: $ARTIFACT_DIR" >&2
    exit 1
fi

# Get version from git tag or environment
VERSION="${GITHUB_REF_NAME:-$(git describe --tags --always 2>/dev/null || echo "unknown")}"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Start JSON
echo "{"
echo "  \"version\": \"$VERSION\","
echo "  \"timestamp\": \"$TIMESTAMP\","
echo "  \"artifacts\": ["

FIRST=true

for FILE in "$ARTIFACT_DIR"/*; do
    NAME=$(basename "$FILE")

    # Skip metadata files
    case "$NAME" in
        *.sha256|*.sig|*.bundle|manifest.json|SHA256SUMS)
            continue
            ;;
    esac

    if [[ ! -f "$FILE" ]]; then
        continue
    fi

    SHA256=$(sha256sum "$FILE" | awk '{print $1}')
    SIZE=$(stat --printf="%s" "$FILE" 2>/dev/null || stat -f "%z" "$FILE" 2>/dev/null)

    # Check for signature artifacts
    HAS_BUNDLE="false"
    if [[ -f "${FILE}.bundle" ]]; then
        HAS_BUNDLE="true"
    fi

    if [[ "$FIRST" != "true" ]]; then
        echo ","
    fi
    FIRST=false

    echo -n "    {"
    echo -n "\"name\": \"$NAME\""
    echo -n ", \"sha256\": \"$SHA256\""
    echo -n ", \"size\": $SIZE"
    if [[ "$HAS_BUNDLE" == "true" ]]; then
        echo -n ", \"bundle\": \"${NAME}.bundle\""
    fi
    echo -n "}"
done

echo ""
echo "  ]"
echo "}"
