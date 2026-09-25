#!/usr/bin/env bash
# Assemble a source-only macOS arm64 archive from separately certified binaries.
# Run under run_with_project_artifacts.sh and run_in_hard_memory_scope.sh on Linux.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
root_dir="$(cd "$script_dir/.." && pwd -P)"
version="${1:-}"
compiler="${2:-}"
package_client="${3:-}"
output_dir="${4:-$root_dir/dist}"

die() { printf 'macOS package: %s\n' "$*" >&2; exit 1; }
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'expected a release version'
[[ -f "$compiler" && ! -L "$compiler" ]] || die 'compiler is missing or unsafe'
[[ -f "$package_client" && ! -L "$package_client" ]] || die 'package client is missing or unsafe'
[[ "$(uname -s)" == Linux ]] || die 'this cross-host packager requires Linux containment'
"$script_dir/run_in_hard_memory_scope.sh" --verify-only >/dev/null || die 'hard scope is not verified'

manifest="$root_dir/releases/compatibility-manifest.json"
python3 "$script_dir/check_compatibility_manifest.py" "$manifest" >/dev/null || die 'invalid compatibility manifest'
[[ "$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["release_version"])' "$manifest")" == "$version" ]] ||
    die 'compatibility manifest version mismatch'
file "$compiler" | grep -Eq 'Mach-O 64-bit arm64 executable' || die 'compiler is not Mach-O arm64'
file "$package_client" | grep -Eq 'Mach-O 64-bit arm64 executable' || die 'package client is not Mach-O arm64'

mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd -P)"
scope="$root_dir/.seen/agent-tools/macos-release-package"
mkdir -p "$scope"
stage="$(mktemp -d "$scope/run.XXXXXX")"
cleanup() {
    local status=$?
    case "$stage" in "$scope"/run.*) rm -rf -- "$stage" ;; esac
    return "$status"
}
trap cleanup EXIT

name="seen-$version-macos-arm64"
payload="$stage/$name"
mkdir -p "$payload/bin" "$payload/lib/seen/std" "$payload/lib/seen/runtime" \
    "$payload/lib/seen/toolchain" "$payload/share/seen/languages" \
    "$payload/share/seen" "$payload/share/doc/seen"
install -m 755 "$compiler" "$payload/bin/seen"
install -m 755 "$package_client" "$payload/bin/seen-pkg"
install -m 644 "$manifest" "$payload/bin/compatibility-manifest.json"

while IFS= read -r -d '' path; do
    case "$path" in
        seen_std/src/*) destination="$payload/lib/seen/std/${path#seen_std/src/}" ;;
        seen_runtime/*.o|seen_runtime/*.sig|seen_runtime/*.a) continue ;;
        seen_runtime/*) destination="$payload/lib/seen/runtime/${path#seen_runtime/}" ;;
        languages/*) destination="$payload/share/seen/languages/${path#languages/}" ;;
        *) continue ;;
    esac
    mkdir -p "$(dirname "$destination")"
    install -m 644 "$root_dir/$path" "$destination"
done < <(git -C "$root_dir" ls-files -z -- seen_std/src seen_runtime languages)

for path in README.md CHANGELOG.md LICENSE; do
    [[ -f "$root_dir/$path" ]] && install -m 644 "$root_dir/$path" "$payload/share/doc/seen/$path"
done
install -m 755 "$script_dir/seen_toolchain.sh" "$payload/lib/seen/toolchain/seen-toolchain.sh"
printf '%s\n' \
    'seen_toolchain_manifest_version=1' \
    'llvm_min_version=19' \
    'llvm_preferred_version=20' \
    'required_tools=clang,opt,llc,llvm-as,ld.lld' \
    'bundle_mode=external' > "$payload/lib/seen/toolchain/manifest.env"
printf 'release_version=%s\nsource_commit=%s\nplatform=macos-arm64\ncompiler_sha256=%s\npackage_client_sha256=%s\n' \
    "$version" "$(git -C "$root_dir" rev-parse HEAD)" \
    "$(sha256sum "$compiler" | awk '{print $1}')" \
    "$(sha256sum "$package_client" | awk '{print $1}')" \
    > "$payload/share/seen/release-provenance.env"

archive="$output_dir/$name.tar.gz"
tar --sort=name --mtime="@$(git -C "$root_dir" show -s --format=%ct HEAD)" \
    --owner=0 --group=0 --numeric-owner -C "$stage" -czf "$archive.tmp" "$name"
mv -f -- "$archive.tmp" "$archive"
tar -tzf "$archive" | awk -v wanted="$name/bin/seen" '$0 == wanted { found = 1 } END { exit !found }' ||
    die 'compiler missing from archive'
tar -tzf "$archive" | awk -v wanted="$name/bin/seen-pkg" '$0 == wanted { found = 1 } END { exit !found }' ||
    die 'package client missing from archive'
sha256sum "$archive"
