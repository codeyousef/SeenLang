#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
REGISTRY_ROOT="$ROOT_DIR/dist/registry"
MANIFEST_PATH=""
SYNC_DIR=""
BASE_URL="https://seen.yousef.codes/packages"

usage() {
    cat <<'USAGE'
publish_registry.sh - Publish a Seen package into a static registry tree.

Usage: scripts/publish_registry.sh [options]

Options:
  --manifest <path>       Package directory or Seen.toml to publish (default: current directory)
  --registry-root <dir>   Local registry output root (default: dist/registry)
  --compiler <path>       Seen compiler binary to use (default: compiler_seen/target/seen)
  --sync-dir <dir>        Optional destination directory to mirror the registry into
  --base-url <url>        Public registry URL for follow-up instructions
  -h, --help              Show this help message

Examples:
  scripts/publish_registry.sh --manifest ./examples/mathx
  scripts/publish_registry.sh --manifest ./examples/mathx --sync-dir ~/site/public/packages
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
        --sync-dir)
            SYNC_DIR="$2"
            shift 2
            ;;
        --base-url)
            BASE_URL="$2"
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

echo "Publishing package into local registry..."
"$COMPILER" pkg publish "$REGISTRY_ROOT" "$MANIFEST_PATH"

if [[ -n "$SYNC_DIR" ]]; then
    mkdir -p "$SYNC_DIR"
    echo "Syncing registry to $SYNC_DIR"
    if command -v rsync >/dev/null 2>&1; then
        rsync -a --delete "$REGISTRY_ROOT"/ "$SYNC_DIR"/
    else
        cp -R "$REGISTRY_ROOT"/. "$SYNC_DIR"/
        echo "rsync not found; copied registry files without deleting stale files in $SYNC_DIR"
    fi
fi

echo ""
echo "Registry ready at: $REGISTRY_ROOT"
if [[ -n "$SYNC_DIR" ]]; then
    echo "Synced copy at:    $SYNC_DIR"
fi
echo "Public URL base:   $BASE_URL"
echo ""
echo "Registry layout:"
echo "  index/<package>.toml"
echo "  archives/<package>/<package>-<version>.seenpkg.tgz"
echo ""
echo "Consumer manifest:"
echo "  [registries]"
echo "  default = \"$BASE_URL\""
