#!/usr/bin/env bash
# Fetch only the three required platform assets from an unpublished draft.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
root_dir="$(cd "$script_dir/.." && pwd -P)"
version="${1:-}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
    echo 'expected release version' >&2
    exit 2
}
release_id="${SEEN_RELEASE_DRAFT_ID:-}"
[[ "$release_id" =~ ^[1-9][0-9]*$ ]] || {
    echo 'numeric draft release ID is required' >&2
    exit 2
}
input_dir="$root_dir/.seen/agent-tools/release-platform-inputs/$version"
mkdir -p "$input_dir"
python3 "$script_dir/release_draft_api.py" download-inputs \
    --version "$version" --release-id "$release_id" --output-dir "$input_dir"
python3 "$script_dir/release_platform_inputs.py" verify --root "$root_dir" \
    --version "$version" --input-dir "$input_dir"
printf '%s\n' "$input_dir"
