#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
REGISTRY_ROOT="$ROOT_DIR/dist/registry"
MANIFEST_PATH=""

usage() {
    cat <<'USAGE'
publish_registry.sh - Build a legacy local-static Seen registry tree.

This helper does not run hosted-v1 ingestion, scanning, delay, or signing.

Usage: scripts/publish_registry.sh [options]

Options:
  --manifest <path>       Package directory or Seen.toml to publish (default: current directory)
  --registry-root <dir>   Local registry output root (default: dist/registry)
  --compiler <path>       Seen compiler binary to use (default: compiler_seen/target/seen)
  -h, --help              Show this help message

Examples:
  scripts/publish_registry.sh --manifest ./examples/mathx
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --manifest)
            MANIFEST_PATH="$2"
            shift 2
            ;;
        --registry-root)
            REGISTRY_ROOT="$2"
            shift 2
            ;;
        --compiler)
            COMPILER="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage
            exit 1
            ;;
    esac
done

if [[ ! -x "$COMPILER" ]]; then
    echo "Compiler not found or not executable: $COMPILER" >&2
    echo "Build it first with ./scripts/safe_rebuild.sh" >&2
    exit 1
fi

if [[ -z "$MANIFEST_PATH" ]]; then
    MANIFEST_PATH="$PWD"
fi

mkdir -p "$REGISTRY_ROOT"

echo "Building legacy local-static package tree (not hosted-v1 publication)..."
"$COMPILER" pkg publish "$REGISTRY_ROOT" "$MANIFEST_PATH"

echo ""
echo "Registry ready at: $REGISTRY_ROOT"
echo ""
echo "Registry layout:"
echo "  index/<package>.toml"
echo "  archives/<package>/<package>-<version>.seenpkg.tgz"
echo ""
echo "Consumer manifest:"
echo "  [registries]"
echo "  default = \"$REGISTRY_ROOT\""
echo ""
echo "Do not expose this tree as the hosted Seen registry writer."
