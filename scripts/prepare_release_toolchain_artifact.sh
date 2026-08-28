#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
ARCHIVE_TOOL="$ROOT_DIR/scripts/release_toolchain_artifact.py"
OUTPUT="${1:-$ROOT_DIR/target/seen-build/release-toolchain.tar.gz}"
BASELINE="${SEEN_RELEASE_CPU_BASELINE:-}"

fail() {
    echo "release-toolchain: core.004b.invalid: $*" >&2
    exit 1
}

[ "$BASELINE" = "x86-64" ] || fail "main CI must certify the portable x86-64 baseline"
[ -f "$ARCHIVE_TOOL" ] && [ ! -L "$ARCHIVE_TOOL" ] || fail "archive tool is missing or unsafe"
[ -z "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ] ||
    fail "release toolchain preparation requires a clean checkout"

# Keep the release-preparation path fail closed on the public CORE-004F/G
# evidence formats. The expensive compiler/package build below remains the one
# authoritative release-prep build; these validators do not rebuild it.
"$ROOT_DIR/tests/misc_root_tests/seen_program_artifacts_contract.sh"
SEEN_CORE_004G_FUZZ_SECONDS=0.01 \
    "$ROOT_DIR/tests/misc_root_tests/seen_program_reproducibility_contract.sh"

commit=$(git -C "$ROOT_DIR" rev-parse HEAD) || fail "could not resolve HEAD"
tree=$(git -C "$ROOT_DIR" rev-parse 'HEAD^{tree}') || fail "could not resolve source tree"
version=$(python3 -c \
    'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["release_version"])' \
    "$ROOT_DIR/releases/compatibility-manifest.json") || fail "could not resolve release version"
case "$version" in
    ''|*[!0-9A-Za-z.+-]*) fail "release version is invalid" ;;
esac

# Exercise the real package and CPU-baseline verification path before making
# the certified compiler transferable. Dry-run mode cannot sign, tag, or upload.
SEEN_RELEASE_DRY_RUN=1 SEEN_RELEASE_CLEAN_DIST=1 \
    "$ROOT_DIR/scripts/run_release_upload.sh" "$version"

python3 "$ARCHIVE_TOOL" create --root "$ROOT_DIR" --output "$OUTPUT" \
    --commit "$commit" --tree "$tree" --cpu-baseline "$BASELINE"
python3 "$ARCHIVE_TOOL" verify --archive "$OUTPUT" \
    --commit "$commit" --tree "$tree" --cpu-baseline "$BASELINE"
echo "Prepared certified release toolchain: $OUTPUT"
