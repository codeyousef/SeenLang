#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_full_release_stamp.XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

REPO="$TMP_DIR/repo"
mkdir -p "$REPO/scripts" "$REPO/bin"
cp "$ROOT_DIR/scripts/build_trace_common.sh" "$REPO/scripts/"

cat > "$REPO/bin/seen" <<'SEEN_EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--version" ]]; then
    echo "Seen 0.10.0"
    exit 0
fi
exit 2
SEEN_EOF
cat > "$REPO/bin/seen-pkg" <<'PKG_EOF'
#!/usr/bin/env bash
exit 0
PKG_EOF
chmod 755 "$REPO/bin/seen" "$REPO/bin/seen-pkg"
printf 'verified source\n' > "$REPO/source.seen"
printf 'target/\n' > "$REPO/.gitignore"

git -C "$REPO" init -q
git -C "$REPO" config user.name "Seen release test"
git -C "$REPO" config user.email "release-test@invalid.example"
git -C "$REPO" add .
git -C "$REPO" commit -qm "fixture"

# shellcheck source=scripts/build_trace_common.sh
source "$REPO/scripts/build_trace_common.sh"
SEEN_PACKAGE_CLIENT_BIN="$REPO/bin/seen-pkg"
export SEEN_PACKAGE_CLIENT_BIN

seen_build_write_full_release_stamp "$REPO" "$REPO/bin/seen" >/dev/null
seen_build_require_full_release_stamp "$REPO" "$REPO/bin/seen"

printf 'dirty source\n' > "$REPO/source.seen"
if seen_build_require_full_release_stamp "$REPO" "$REPO/bin/seen" >/dev/null 2>&1; then
    echo "full release stamp accepted a dirty source tree" >&2
    exit 1
fi

git -C "$REPO" add source.seen
git -C "$REPO" commit -qm "change source without rebuilding"
if seen_build_require_full_release_stamp "$REPO" "$REPO/bin/seen" >/dev/null 2>&1; then
    echo "full release stamp accepted a different committed source tree" >&2
    exit 1
fi

echo "full release stamp source-tree binding test passed"
