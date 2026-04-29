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
        echo "Seen Language 0.7.2"
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
        echo "Usage: seen <check|compile>"
        exit 1
        ;;
esac
FAKE_EOF
chmod +x "$FAKE_COMPILER"

DIST_DIR="$TMP_DIR/dist"
"$ROOT_DIR/scripts/build_release.sh" \
    --version 0.7.2 \
    --output-dir "$DIST_DIR" \
    --compiler "$FAKE_COMPILER" \
    --cpu-baseline x86-64 \
    --artifact-suffix linux-x64 \
    --skip-verify >/dev/null

TARBALL="$DIST_DIR/seen-0.7.2-linux-x64.tar.gz"
test -f "$TARBALL"

"$ROOT_DIR/scripts/verify_release_cpu_baseline.sh" --cpu-baseline x86-64 "$TARBALL" >/dev/null

EXTRACT_DIR="$TMP_DIR/extract"
mkdir -p "$EXTRACT_DIR"
tar -xzf "$TARBALL" -C "$EXTRACT_DIR"
PACKAGE_DIR="$EXTRACT_DIR/seen-0.7.2-linux-x64"

PREFIX="$TMP_DIR/prefix"
mkdir -p "$PREFIX/bin"
SYMLINK_TARGET="$TMP_DIR/original-target"
printf 'original target content\n' > "$SYMLINK_TARGET"
ln -s "$SYMLINK_TARGET" "$PREFIX/bin/seen"

(cd "$PACKAGE_DIR" && ./install.sh "$PREFIX" >/dev/null)

if [[ "$(cat "$SYMLINK_TARGET")" != "original target content" ]]; then
    echo "install.sh followed the existing seen symlink and overwrote its target" >&2
    exit 1
fi

if [[ -L "$PREFIX/bin/seen" ]]; then
    echo "install.sh left the seen destination as a symlink" >&2
    exit 1
fi

if ! "$PREFIX/bin/seen" --version | grep -q '0.7.2'; then
    echo "installed seen binary did not come from the release package" >&2
    exit 1
fi

grep -qx 'cpu-baseline=x86-64' "$PACKAGE_DIR/share/doc/seen/release-cpu-baseline.txt"

echo "release packaging symlink replacement test passed"
