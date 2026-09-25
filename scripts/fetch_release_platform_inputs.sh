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
tag="v$version"
[[ "$(gh release view "$tag" --repo codeyousef/SeenLang --json isDraft --jq .isDraft)" == true ]] || {
    echo 'release is absent or is not a draft' >&2
    exit 1
}
expected_assets="$(printf '%s\n' \
    "seen-$version-macos-arm64.tar.gz" \
    "seen-$version-windows-x64.zip" \
    "Seen-$version-windows-x64-setup.exe" \
    "seen-$version-platform-inputs.json" | LC_ALL=C sort)"
actual_assets="$(gh release view "$tag" --repo codeyousef/SeenLang \
    --json assets --jq '.assets[].name' | LC_ALL=C sort)"
[[ "$actual_assets" == "$expected_assets" ]] || {
    echo 'staged draft has an unexpected or missing asset' >&2
    exit 1
}
input_dir="$root_dir/.seen/agent-tools/release-platform-inputs/$version"
mkdir -p "$input_dir"
for name in \
    "seen-$version-macos-arm64.tar.gz" \
    "seen-$version-windows-x64.zip" \
    "Seen-$version-windows-x64-setup.exe" \
    "seen-$version-platform-inputs.json"; do

    [[ ! -e "$input_dir/$name" ]] || {
        echo "stale local platform input exists: $name" >&2
        exit 1
    }
    gh release download "$tag" --repo codeyousef/SeenLang \
        --pattern "$name" --dir "$input_dir"
done
python3 "$script_dir/release_platform_inputs.py" verify --root "$root_dir" \
    --version "$version" --input-dir "$input_dir"
printf '%s\n' "$input_dir"
