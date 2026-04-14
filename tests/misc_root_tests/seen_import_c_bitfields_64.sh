#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_DIR="$(mktemp -d /tmp/seen_import_c_bitfields_64.XXXXXX)"
HEADER="$TMP_DIR/bitfields_64.h"
OUT="$TMP_DIR/bitfields_64_bindings.seen"

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

cat >"$HEADER" <<'H'
#include <stdint.h>

typedef struct WideBits {
    uint64_t low:40;
    uint64_t high:8;
} WideBits;

void use_wide_bits(WideBits *value);
H

"$COMPILER" import-c "$HEADER" | sed -n '/^\/\/ Auto-generated/,$p' >"$OUT"

WIDE_BLOCK="$(sed -n '/^class WideBits {$/,/^}$/p' "$OUT")"
if ! printf '%s\n' "$WIDE_BLOCK" | grep -Fqx '    var _bitfield_storage_0: UInt64'; then
    echo "FAIL: import-c should emit UInt64 backing storage for widened bitfield groups"
    exit 1
fi

if [ "$(grep -Fxc 'fun WideBits_get_low(recordPtr: *WideBits) r: Int {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit getter helpers for 64-bit storage unit bitfields"
    exit 1
fi

if [ "$(grep -Fxc 'fun WideBits_set_high(recordPtr: *WideBits, value: Int) r: Void {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit setter helpers for 64-bit storage unit bitfields"
    exit 1
fi

if grep -Fq 'WARNING: helper omitted for unsupported bitfield layout' "$OUT"; then
    echo "FAIL: import-c should not skip helpers for supported 64-bit storage bitfields"
    exit 1
fi

cat >>"$OUT" <<'H'

fun main() r: Void {
}
H

"$COMPILER" compile "$OUT" "$TMP_DIR/bitfields_64_probe" --fast >/dev/null

if [ "$(grep -Fxc 'extern fun use_wide_bits(arg0: *WideBits) r: Void' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve typed pointers for widened bitfield-backed records"
    exit 1
fi

echo "PASS: import-c emits helpers for 64-bit storage-unit bitfields"
