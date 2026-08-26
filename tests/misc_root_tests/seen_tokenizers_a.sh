#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_CLIENT="$ROOT_DIR/tools/seen-pkg/bin/seen-pkg"
PACKAGE_ARTIFACT_ROOT="${SEEN_ARTIFACT_ROOT:-$ROOT_DIR/.seen/agent-tools}"
python3 "$ROOT_DIR/packages/seen_tokenizers/scripts/generate_unicode_tables.py" \
    --output "$ROOT_DIR/packages/seen_tokenizers/src/unicode_tables.seen" --check
[ -x "$PACKAGE_CLIENT" ] || {
    echo "FAIL: v0.15 package client is unavailable" >&2
    exit 1
}
mkdir -p "$PACKAGE_ARTIFACT_ROOT/tokenizers-package"
ARCHIVE="$PACKAGE_ARTIFACT_ROOT/tokenizers-package/seen-tokenizers-0.1.0.tar.gz"
ARCHIVE_LIST="$PACKAGE_ARTIFACT_ROOT/tokenizers-package/contents.txt"
"$PACKAGE_CLIENT" --expect-version 0.15.0 pack \
    "$ROOT_DIR/packages/seen_tokenizers" --output "$ARCHIVE" >/dev/null
tar -tzf "$ARCHIVE" >"$ARCHIVE_LIST"
for required in Seen.toml README.md LICENSE src/mod.seen src/unicode_tables.seen \
    src/json.seen src/bpe.seen; do
    grep -Fxq "$required" "$ARCHIVE_LIST" || {
        echo "FAIL: seen_tokenizers archive omitted $required" >&2
        exit 1
    }
done
bash "$ROOT_DIR/tests/misc_root_tests/seen_tokenizers_b.sh"
echo "PASS: SEEN-LIB-004A strict tokenizer JSON and Unicode assets"
