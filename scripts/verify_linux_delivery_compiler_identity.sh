#!/usr/bin/env bash
# Prove that every produced Linux delivery embeds the exact signed compiler
# component and that the portable installer preserves those bytes.

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() {
    echo "linux-delivery-provenance: invalid: $*" >&2
    exit 1
}

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <version> <dist-dir>" >&2
    exit 2
fi
version=$1
dist_dir=$2
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] ||
    fail "invalid version"
[ -d "$dist_dir" ] && [ ! -L "$dist_dir" ] || fail "dist directory is unsafe"
dist_dir=$(cd -P -- "$dist_dir" && pwd -P)

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER"
seen_artifact_root_init "$ROOT_DIR"
scope=$(seen_artifact_scope_init linux-delivery-provenance)
work=$(seen_artifact_mktemp_dir "$scope" run)
cleanup() {
    local status=$?
    case "$work" in
        "$scope"/run.*)
            [ ! -L "$work" ] && rm -rf -- "$work" || status=1
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT

component="$dist_dir/seen-compiler-$version-linux-x64"
[ -f "$component" ] && [ -x "$component" ] && [ ! -L "$component" ] ||
    fail "signed compiler component is missing or unsafe"

verify_tree() {
    local label=$1
    local tree=$2
    local compiler=$3
    local manifest=$4
    local verifier=$5
    [ -f "$tree/$compiler" ] && [ ! -L "$tree/$compiler" ] ||
        fail "$label compiler is missing or unsafe"
    [ -f "$tree/$manifest" ] && [ ! -L "$tree/$manifest" ] ||
        fail "$label provenance manifest is missing or unsafe"
    [ -x "$tree/$verifier" ] && [ ! -L "$tree/$verifier" ] ||
        fail "$label provenance verifier is missing or unsafe"
    cmp -s "$component" "$tree/$compiler" ||
        fail "$label compiler differs from signed component"
    "$tree/$verifier" "$tree/$manifest" "$tree/$compiler" "$version" >/dev/null ||
        fail "$label compiler provenance verification failed"
    echo "linux-delivery-provenance: verified $label"
}

archive="$dist_dir/seen-$version-linux-x64.tar.gz"
[ -f "$archive" ] && [ ! -L "$archive" ] || fail "complete Linux archive is missing"
archive_tree="$work/archive"
mkdir -p "$archive_tree"
tar -xzf "$archive" --strip-components=1 -C "$archive_tree"
verify_tree archive "$archive_tree" bin/seen share/seen/compiler-provenance.env \
    lib/seen/toolchain/verify-compiler-provenance.sh

# Clean-prefix installation is the canonical clean-host smoke.  The installer
# performs both pre-install source verification and installed-byte readback.
prefix="$work/prefix"
(cd "$archive_tree" && SEEN_SKIP_TOOLCHAIN=1 ./install.sh "$prefix" >/dev/null)
verify_tree installed-prefix "$prefix" bin/seen share/seen/compiler-provenance.env \
    lib/seen/toolchain/verify-compiler-provenance.sh

deb="$dist_dir/seen-lang_${version}_amd64.deb"
if [ -f "$deb" ]; then
    command -v dpkg-deb >/dev/null 2>&1 || fail "dpkg-deb is required to inspect the DEB"
    deb_tree="$work/deb"
    mkdir -p "$deb_tree"
    dpkg-deb -x "$deb" "$deb_tree"
    verify_tree deb "$deb_tree" usr/bin/seen usr/share/seen/compiler-provenance.env \
        usr/lib/seen/toolchain/verify-compiler-provenance.sh
fi

rpm="$dist_dir/seen-lang-$version-1.x86_64.rpm"
if [ -f "$rpm" ]; then
    command -v rpm2cpio >/dev/null 2>&1 || fail "rpm2cpio is required to inspect the RPM"
    command -v cpio >/dev/null 2>&1 || fail "cpio is required to inspect the RPM"
    rpm_tree="$work/rpm"
    mkdir -p "$rpm_tree"
    (cd "$rpm_tree" && rpm2cpio "$rpm" | cpio -idm --quiet)
    rpm_compiler=$(find "$rpm_tree/usr" -type f -path '*/bin/seen' -print -quit)
    rpm_manifest=$(find "$rpm_tree/usr" -type f -path '*/share/seen/compiler-provenance.env' -print -quit)
    rpm_verifier=$(find "$rpm_tree/usr" -type f -path '*/lib*/seen/toolchain/verify-compiler-provenance.sh' -print -quit)
    [ -n "$rpm_compiler" ] && [ -n "$rpm_manifest" ] && [ -n "$rpm_verifier" ] ||
        fail "RPM provenance payload is incomplete"
    verify_tree rpm "$rpm_tree" "${rpm_compiler#"$rpm_tree/"}" \
        "${rpm_manifest#"$rpm_tree/"}" "${rpm_verifier#"$rpm_tree/"}"
fi

appimage="$dist_dir/SeenLanguage-$version-x86_64.AppImage"
if [ -f "$appimage" ]; then
    [ -x "$appimage" ] || fail "AppImage is not executable"
    app_work="$work/appimage"
    mkdir -p "$app_work"
    (cd "$app_work" && "$appimage" --appimage-extract >/dev/null)
    verify_tree appimage "$app_work/squashfs-root" usr/bin/seen \
        usr/share/seen/compiler-provenance.env \
        usr/lib/seen/toolchain/verify-compiler-provenance.sh
fi

echo "PASS: all Linux delivery compilers match the signed component"
