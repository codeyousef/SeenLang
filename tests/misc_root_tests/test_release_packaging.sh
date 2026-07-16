#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_release_packaging.XXXXXX)"

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

FAKE_COMPILER="$TMP_DIR/seen"
cat > "$FAKE_COMPILER" <<'FAKE_EOF'
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
    --version)
        echo "Seen 0.10.0"
        ;;
    pkg)
        case "${2:-}" in
            prebuild)
                artifact_dir="${4:-}"
                if [[ -z "$artifact_dir" ]]; then
                    echo "missing artifact dir" >&2
                    exit 1
                fi
                mkdir -p "$artifact_dir/objects"
                printf 'objects/module_0.o\t%s\n' "${3:-}/src/value.seen" > "$artifact_dir/objects.tsv"
                printf 'release_prebuild_value\t%s\n' "${3:-}/src/value.seen" > "$artifact_dir/interface.index.tsv"
                printf 'fake object\n' > "$artifact_dir/objects/module_0.o"
                ;;
            *)
                echo "Usage: seen pkg fetch [project-dir-or-manifest]"
                echo "       seen pkg pack [project-dir-or-manifest] [output]"
                echo "       seen pkg prebuild [project-dir-or-manifest] [output-dir]"
                echo "       seen pkg publish <registry-dir> [project-dir-or-manifest]"
                ;;
        esac
        ;;
    check)
        test -f "${2:-}"
        ;;
    compile)
        output="${3:-}"
        if [[ -z "$output" ]]; then
            echo "missing output" >&2
            exit 1
        fi
        cat > "$output" <<'OUT_EOF'
#!/usr/bin/env bash
echo release smoke
OUT_EOF
        chmod +x "$output"
        ;;
    *)
        echo "Usage: seen <check|compile|pkg>"
        exit 1
        ;;
esac
FAKE_EOF
chmod +x "$FAKE_COMPILER"

FAKE_PACKAGE_CLIENT="$TMP_DIR/seen-pkg"
cat > "$FAKE_PACKAGE_CLIENT" <<'PKG_EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--expect-version" && "${2:-}" == "0.10.0" && "${3:-}" == "version" ]]; then
    echo "seen-pkg 0.10.0"
    exit 0
fi
echo "fake seen package client"
PKG_EOF
chmod +x "$FAKE_PACKAGE_CLIENT"

DIST_DIR="$TMP_DIR/dist"
ARTIFACT_CACHE_ROOT="$TMP_DIR/artifact-cache"
mkdir -p "$DIST_DIR"
printf 'stale release artifact\n' > "$DIST_DIR/seen-0.9.2-linux-x64.tar.gz"

SEEN_PACKAGE_CLIENT_BIN="$FAKE_PACKAGE_CLIENT" \
SEEN_RELEASE_ARTIFACT_CACHE_ROOT="$ARTIFACT_CACHE_ROOT" "$ROOT_DIR/scripts/build_release.sh" \
    --version 0.10.0 \
    --output-dir "$DIST_DIR" \
    --compiler "$FAKE_COMPILER" \
    --cpu-baseline x86-64 \
    --artifact-suffix linux-x64 \
    --skip-verify >/dev/null

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"
SEEN_PACKAGE_CLIENT_BIN="$FAKE_PACKAGE_CLIENT" \
SEEN_RELEASE_ARTIFACT_CACHE_ROOT="$ARTIFACT_CACHE_ROOT" "$ROOT_DIR/scripts/build_release.sh" \
    --version 0.10.0 \
    --output-dir "$DIST_DIR" \
    --compiler "$FAKE_COMPILER" \
    --cpu-baseline x86-64 \
    --artifact-suffix linux-x64 \
    --skip-verify >/dev/null

if [[ -e "$DIST_DIR/seen-0.9.2-linux-x64.tar.gz" ]]; then
    echo "release artifact cache restored a stale version" >&2
    exit 1
fi

TARBALL="$DIST_DIR/seen-0.10.0-linux-x64.tar.gz"
test -f "$TARBALL"

"$ROOT_DIR/scripts/verify_release_cpu_baseline.sh" --cpu-baseline x86-64 "$TARBALL" >/dev/null

MISMATCH_DIR="$TMP_DIR/version-mismatch"
mkdir -p "$MISMATCH_DIR"
tar -xzf "$TARBALL" -C "$MISMATCH_DIR"
MISMATCH_PACKAGE="$MISMATCH_DIR/seen-0.10.0-linux-x64"
sed -i 's/Seen 0\.10\.0/Seen 9.9.9/' "$MISMATCH_PACKAGE/bin/seen"
MISMATCH_TARBALL="$TMP_DIR/seen-version-mismatch.tar.gz"
tar -czf "$MISMATCH_TARBALL" -C "$MISMATCH_DIR" "$(basename "$MISMATCH_PACKAGE")"
set +e
mismatch_output="$("$ROOT_DIR/scripts/verify_release_cpu_baseline.sh" \
    --cpu-baseline x86-64 "$MISMATCH_TARBALL" 2>&1)"
mismatch_status=$?
set -e
if [[ "$mismatch_status" -eq 0 ]]; then
    echo "release verifier accepted mismatched compiler and release versions" >&2
    exit 1
fi
if ! grep -Fq "Compiler version mismatch: release metadata expects 'Seen 0.10.0', got 'Seen 9.9.9'" \
    <<<"$mismatch_output"; then
    echo "$mismatch_output" >&2
    echo "release verifier did not report the compiler version mismatch" >&2
    exit 1
fi

MIN_SCAN_PATH="$TMP_DIR/min_scan_path"
mkdir -p "$MIN_SCAN_PATH"
for tool in bash tar gzip find head grep mktemp rm cat chmod basename mkdir sed; do
    tool_path="$(command -v "$tool")"
    ln -s "$tool_path" "$MIN_SCAN_PATH/$tool"
done

PATH="$MIN_SCAN_PATH" "$ROOT_DIR/scripts/verify_release_cpu_baseline.sh" \
    --cpu-baseline x86-64 "$TARBALL" >/dev/null

EXTRACT_DIR="$TMP_DIR/extract"
mkdir -p "$EXTRACT_DIR"
tar -xzf "$TARBALL" -C "$EXTRACT_DIR"
PACKAGE_DIR="$EXTRACT_DIR/seen-0.10.0-linux-x64"

PREFIX="$TMP_DIR/prefix"
mkdir -p "$PREFIX/bin"
SYMLINK_TARGET="$TMP_DIR/original-target"
printf 'original target content\n' > "$SYMLINK_TARGET"
ln -s "$SYMLINK_TARGET" "$PREFIX/bin/seen"
PKG_SYMLINK_TARGET="$TMP_DIR/original-pkg-target"
printf 'original package target content\n' > "$PKG_SYMLINK_TARGET"
ln -s "$PKG_SYMLINK_TARGET" "$PREFIX/bin/seen-pkg"

(cd "$PACKAGE_DIR" && SEEN_SKIP_TOOLCHAIN=1 ./install.sh "$PREFIX" >/dev/null)

if [[ "$(cat "$SYMLINK_TARGET")" != "original target content" ]]; then
    echo "install.sh followed the existing seen symlink and overwrote its target" >&2
    exit 1
fi

if [[ -L "$PREFIX/bin/seen" ]]; then
    echo "install.sh left the seen destination as a symlink" >&2
    exit 1
fi

if [[ "$(cat "$PKG_SYMLINK_TARGET")" != "original package target content" ]]; then
    echo "install.sh followed the existing seen-pkg symlink and overwrote its target" >&2
    exit 1
fi
if [[ -L "$PREFIX/bin/seen-pkg" || ! -x "$PREFIX/bin/seen-pkg" ]]; then
    echo "install.sh did not atomically install seen-pkg" >&2
    exit 1
fi

if ! "$PREFIX/bin/seen" --version | grep -q '0.10.0'; then
    echo "installed seen binary did not come from the release package" >&2
    exit 1
fi

grep -qx 'cpu-baseline=x86-64' "$PACKAGE_DIR/share/doc/seen/release-cpu-baseline.txt"
test -f "$PACKAGE_DIR/share/doc/seen/CHANGELOG.md"
grep -Fq '## [0.10.0] - 2026-07-16' "$PACKAGE_DIR/share/doc/seen/CHANGELOG.md"
grep -qx 'llvm_min_version=18' "$PACKAGE_DIR/lib/seen/toolchain/manifest.env"
test -x "$PACKAGE_DIR/lib/seen/toolchain/seen-toolchain.sh"
test -f "$PACKAGE_DIR/share/doc/seen/toolchain-dependencies.txt"
test -x "$PREFIX/lib/seen/toolchain/seen-toolchain.sh"

echo "release packaging symlink replacement test passed"
