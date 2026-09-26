#!/usr/bin/env bash
# Stage exact-commit macOS and Windows inputs in a private draft release.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
root_dir="$(cd "$script_dir/.." && pwd -P)"
version="${1:-}"
input_dir="${2:-}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "$input_dir" == /* ]] || {
    echo 'Usage: stage_release_platform_inputs.sh VERSION ABSOLUTE_INPUT_DIR' >&2
    exit 2
}
tag="v$version"
[[ -d "$input_dir" ]] || { echo 'platform input directory is missing' >&2; exit 1; }
input_dir="$(cd "$input_dir" && pwd -P)"
[[ "$(git -C "$root_dir" status --porcelain)" == '' ]] || {
    echo 'release staging requires a clean exact-commit checkout' >&2
    exit 1
}
source "$script_dir/release_tag_policy.sh"
commit="$(git -C "$root_dir" rev-parse HEAD)"
seen_release_verify_published_tag "$root_dir" "$tag" "$commit" codeyousef/SeenLang
if [[ ! -e "$input_dir/seen-$version-platform-inputs.json" ]]; then
    python3 "$script_dir/release_platform_inputs.py" create --root "$root_dir" \
        --version "$version" --input-dir "$input_dir"
fi
python3 "$script_dir/release_platform_inputs.py" verify --root "$root_dir" \
    --version "$version" --input-dir "$input_dir"
if gh release view "$tag" --repo codeyousef/SeenLang >/dev/null 2>&1; then
    echo 'release already exists; refusing to alter it' >&2
    exit 1
fi
gh release create "$tag" --repo codeyousef/SeenLang --verify-tag --draft \
    --title "Seen Language $version" \
    --notes "Three-platform inputs staged for exact-commit signing and publication; not yet released." \
    "$input_dir/seen-$version-macos-arm64.tar.gz" \
    "$input_dir/seen-$version-windows-x64.zip" \
    "$input_dir/Seen-$version-windows-x64-setup.exe" \
    "$input_dir/seen-$version-platform-inputs.json"
[[ "$(gh release view "$tag" --repo codeyousef/SeenLang --json isDraft --jq .isDraft)" == true ]] || {
    echo 'staged release is not a draft' >&2
    exit 1
}
release_id="$(gh api 'repos/codeyousef/SeenLang/releases?per_page=100' \
    --jq ".[] | select(.tag_name == \"$tag\" and .draft == true) | .id")" || {
    echo 'could not resolve the numeric staged draft release ID' >&2
    exit 1
}
[[ "$release_id" =~ ^[1-9][0-9]*$ ]] || {
    echo 'draft release ID is absent or ambiguous' >&2
    exit 1
}
echo "PASS: staged $tag platform inputs in draft release ID $release_id"
