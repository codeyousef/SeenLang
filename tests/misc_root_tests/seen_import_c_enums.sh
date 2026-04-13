#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_DIR="/tmp/seen_import_c_enums"
HEADER="$TMP_DIR/import_enums.h"
OUT="$TMP_DIR/import.out"

rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

cat >"$HEADER" <<'EOF'
enum VkResultLike {
    VK_FOO = 0,
    VK_BAR = -4,
    VK_BIG = 1000001003,
};
EOF

"$COMPILER" import-c "$HEADER" >"$OUT"

grep -q '^let VK_FOO: Int = 0$' "$OUT" || {
    echo "FAIL: import-c did not emit VK_FOO enum constant"
    exit 1
}

grep -q '^let VK_BAR: Int = -4$' "$OUT" || {
    echo "FAIL: import-c did not emit VK_BAR enum constant"
    exit 1
}

grep -q '^let VK_BIG: Int = 1000001003$' "$OUT" || {
    echo "FAIL: import-c did not emit VK_BIG enum constant"
    exit 1
}

echo "PASS: import-c emits enum constants"
