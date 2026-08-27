#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_CLIENT="${SEEN_PACKAGE_CLIENT:-$ROOT_DIR/compiler_seen/target/seen-pkg}"
PACKAGE_ARTIFACT_ROOT="${SEEN_ARTIFACT_ROOT:-$ROOT_DIR/.seen/agent-tools}"
UNICODE_CHECK_ROOT="$PACKAGE_ARTIFACT_ROOT/tokenizers-unicode-check"
mkdir -p "$UNICODE_CHECK_ROOT/python-shadow"
printf '%s\n' 'raise RuntimeError("host unicodedata must not be imported in check mode")' \
    > "$UNICODE_CHECK_ROOT/python-shadow/unicodedata.py"
PYTHONPATH="$UNICODE_CHECK_ROOT/python-shadow" \
    python3 "$ROOT_DIR/packages/seen_tokenizers/scripts/generate_unicode_tables.py" \
        --output "$ROOT_DIR/packages/seen_tokenizers/src/unicode_tables.seen" --check
cp "$ROOT_DIR/packages/seen_tokenizers/src/unicode_tables.seen" \
    "$UNICODE_CHECK_ROOT/corrupt-unicode-tables.seen"
printf '\n' >> "$UNICODE_CHECK_ROOT/corrupt-unicode-tables.seen"
if PYTHONPATH="$UNICODE_CHECK_ROOT/python-shadow" \
    python3 "$ROOT_DIR/packages/seen_tokenizers/scripts/generate_unicode_tables.py" \
        --output "$UNICODE_CHECK_ROOT/corrupt-unicode-tables.seen" --check \
        >"$UNICODE_CHECK_ROOT/corrupt.out" 2>"$UNICODE_CHECK_ROOT/corrupt.err"; then

    echo "FAIL: Unicode asset checker accepted corrupted pinned tables" >&2
    exit 1
fi
grep -Fq 'generated Unicode tables are stale: expected sha256' \
    "$UNICODE_CHECK_ROOT/corrupt.err" || {
    echo "FAIL: Unicode asset checker omitted the pinned-digest diagnostic" >&2
    exit 1
}
[ -x "$PACKAGE_CLIENT" ] || {
    echo "FAIL: verified v0.15 package client is unavailable at $PACKAGE_CLIENT" >&2
    exit 1
}
mkdir -p "$PACKAGE_ARTIFACT_ROOT/tokenizers-package"
ARCHIVE="$PACKAGE_ARTIFACT_ROOT/tokenizers-package/seen-tokenizers-0.1.0.tar.gz"
ARCHIVE_LIST="$PACKAGE_ARTIFACT_ROOT/tokenizers-package/contents.txt"
"$PACKAGE_CLIENT" --expect-version 0.16.0 pack \
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
