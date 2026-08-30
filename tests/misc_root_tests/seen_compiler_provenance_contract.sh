#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
VERIFY="$ROOT_DIR/scripts/verify_compiler_provenance.sh"
DELIVERY_VERIFY="$ROOT_DIR/scripts/verify_linux_delivery_compiler_identity.sh"
BUILD_RELEASE="$ROOT_DIR/scripts/build_release.sh"
RELEASE_UPLOAD="$ROOT_DIR/scripts/build_and_upload_release.sh"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"

fail() { echo "FAIL: compiler provenance contract: $*" >&2; exit 1; }
[ -x "$VERIFY" ] && [ ! -L "$VERIFY" ] || fail "verifier is missing or unsafe"
[ -x "$DELIVERY_VERIFY" ] && [ ! -L "$DELIVERY_VERIFY" ] ||
    fail "delivery verifier is missing or unsafe"
bash -n "$VERIFY" "$DELIVERY_VERIFY" "$BUILD_RELEASE" "$RELEASE_UPLOAD" || fail syntax

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER"
seen_artifact_root_init "$ROOT_DIR" || fail "artifact root"
scope=$(seen_artifact_scope_init compiler-provenance-contract) || fail scope
work=$(seen_artifact_mktemp_dir "$scope" run) || fail work
cleanup() {
    local status=$?
    case "$work" in "$scope"/run.*) [ ! -L "$work" ] && rm -rf -- "$work" || status=1 ;; *) status=1 ;; esac
    exit "$status"
}
trap cleanup EXIT

compiler="$work/seen"
cat > "$compiler" <<'COMPILER'
#!/usr/bin/env bash
if [ "${1:-}" = --version ]; then echo 'Seen 9.8.7'; exit 0; fi
exit 1
COMPILER
chmod 755 "$compiler"
digest=$(sha256sum "$compiler" | awk '{print $1}')
size=$(wc -c < "$compiler" | tr -d ' ')
manifest="$work/compiler-provenance.env"
cat > "$manifest" <<EOF
seen_compiler_provenance_version=1
release_version=9.8.7
source_commit=1111111111111111111111111111111111111111
source_asset=seen-compiler-9.8.7-linux-x64
compiler_sha256=$digest
compiler_size=$size
compiler_build_id=none
cpu_baseline=x86-64
EOF
"$VERIFY" "$manifest" "$compiler" 9.8.7 >/dev/null || fail happy

cp "$compiler" "$work/tampered"
printf '\n' >> "$work/tampered"
chmod 755 "$work/tampered"
if "$VERIFY" "$manifest" "$work/tampered" 9.8.7 >/dev/null 2>&1; then
    fail "tampered compiler was accepted"
fi
if "$VERIFY" "$manifest" "$compiler" 9.8.8 >/dev/null 2>&1; then
    fail "mismatched release version was accepted"
fi
cp "$manifest" "$work/duplicate.env"
printf 'compiler_sha256=%s\n' "$digest" >> "$work/duplicate.env"
if "$VERIFY" "$work/duplicate.env" "$compiler" 9.8.7 >/dev/null 2>&1; then
    fail "duplicate manifest key was accepted"
fi

grep -Fq 'cp "$STAGING/bin/seen" "$COMPILER_COMPONENT"' "$BUILD_RELEASE" ||
    fail "signed component does not consume canonical package bytes"
grep -Fq 'verify_linux_delivery_compiler_identity.sh' "$RELEASE_UPLOAD" ||
    fail "release uploader omits delivery identity gate"
for installer in \
    "$ROOT_DIR/installer/scripts/install.sh" \
    "$ROOT_DIR/installer/linux/build-deb.sh" \
    "$ROOT_DIR/installer/linux/build-rpm.sh" \
    "$ROOT_DIR/installer/linux/build-appimage.sh"; do
    grep -Fq 'verify-compiler-provenance.sh' "$installer" ||
        fail "installer omits provenance verification: $installer"
done

echo "PASS: compiler provenance contract"
