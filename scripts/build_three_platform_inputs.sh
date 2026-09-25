#!/usr/bin/env bash
# Build both non-Linux release inputs from one clean, contained source commit.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
root_dir="$(cd "$script_dir/.." && pwd -P)"
version="${1:-}"
builder="${SEEN_RELEASE_CROSS_BUILDER:-$root_dir/compiler_seen/target/seen}"
output_dir="$root_dir/dist/platform-input"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
    echo 'expected release version' >&2
    exit 2
}
[[ -x "$builder" && ! -L "$builder" ]] || {
    echo 'release cross-builder is missing or unsafe' >&2
    exit 1
}
[[ -x "${SEEN_GO:-}" ]] || { echo 'SEEN_GO must point to pinned Go' >&2; exit 1; }
command -v zip >/dev/null && command -v unzip >/dev/null &&
    command -v 7z >/dev/null || {
    echo 'zip, unzip, and 7z are required for Windows release verification' >&2
    exit 1
}
[[ "$(git -C "$root_dir" status --porcelain)" == '' ]] || {
    echo 'release cross-build requires a clean exact-commit checkout' >&2
    exit 1
}
[[ "$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["release_version"])' \
    "$root_dir/releases/compatibility-manifest.json")" == "$version" ]] || {
    echo 'compatibility manifest version differs from requested release' >&2
    exit 1
}

if [[ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ]]; then
    exec "$script_dir/run_with_project_artifacts.sh" three-platform-release-inputs \
        --keep-on-failure -- "$script_dir/run_in_hard_memory_scope.sh" \
        --label 'three-platform cross-build inputs' --timeout-secs 5400 -- "$0" "$@"
fi
"$script_dir/run_in_hard_memory_scope.sh" --verify-only >/dev/null || exit 1
[[ "${SEEN_JOBS:-1}" == 1 && "${SEEN_OPT_JOBS:-1}" == 1 ]] || {
    echo 'release cross-build workers must be serial' >&2
    exit 1
}
export SEEN_JOBS=1 SEEN_OPT_JOBS=1 SEEN_PACKAGE_JOBS=1 SEEN_NO_FORK=1
mkdir -p "$output_dir"
SEEN_WINDOWS_COMPILER_BIN="$builder" \
    "$script_dir/build_windows_installer.sh" "$version" --force-compile
installer="$root_dir/installer/windows/output/Seen-$version-windows-x64-setup.exe"
7z l "$installer" | awk '
    /bin\/compatibility-manifest\.json$/ { manifest = 1 }
    /share\/seen\/release-provenance\.env$/ { provenance = 1 }
    /lib\/seen\/runtime\/seen_runtime\.c$/ { runtime = 1 }
    END { exit !(manifest && provenance && runtime) }
' || { echo 'Windows installer lacks required manifest, provenance, or runtime source' >&2; exit 1; }
cp -- "$root_dir/target-windows/seen-$version-windows-x64.zip" \
    "$installer" \
    "$output_dir/"
SEEN_MACOS_BUILDER="$builder" SEEN_MACOS_OUTPUT_DIR="$output_dir" \
    "$script_dir/build_macos_arm64_release.sh" "$version"
python3 "$script_dir/release_platform_inputs.py" create --root "$root_dir" \
    --version "$version" --input-dir "$output_dir"
python3 "$script_dir/release_platform_inputs.py" verify --root "$root_dir" \
    --version "$version" --input-dir "$output_dir"
echo "PASS: macOS arm64 and Windows x64 release inputs for $(git -C "$root_dir" rev-parse HEAD)"
