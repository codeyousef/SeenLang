#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_DIR="$(mktemp -d /tmp/seen_import_c_bitfields.XXXXXX)"
HEADER="$TMP_DIR/bitfields.h"
FULL_OUT="$TMP_DIR/bitfields_full.out"
OUT="$TMP_DIR/bitfields_bindings.seen"

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT

cat >"$HEADER" <<'EOF'
typedef struct Flags {
    unsigned int a:1;
    unsigned int b:3;
    unsigned int c:4;
    unsigned int d:8;
    unsigned int e:16;
} Flags;

typedef struct SignedFlags {
    int delta:4;
    unsigned int enabled:1;
} SignedFlags;

void use_flags(Flags *flags, SignedFlags *signed_flags);
EOF

"$COMPILER" import-c "$HEADER" >"$FULL_OUT"
sed -n '/^\/\/ Auto-generated/,$p' "$FULL_OUT" >"$OUT"

FLAGS_BLOCK="$(sed -n '/^class Flags {$/,/^}$/p' "$OUT")"
SIGNED_FLAGS_BLOCK="$(sed -n '/^class SignedFlags {$/,/^}$/p' "$OUT")"

if ! printf '%s\n' "$FLAGS_BLOCK" | grep -Fqx '    var _bitfield_storage_0: UInt32'; then
    echo "FAIL: import-c should emit backing storage for unsigned bitfield groups"
    exit 1
fi

if ! printf '%s\n' "$SIGNED_FLAGS_BLOCK" | grep -Fqx '    var _bitfield_storage_0: UInt32'; then
    echo "FAIL: import-c should emit backing storage for signed bitfield groups"
    exit 1
fi

if printf '%s\n%s\n' "$FLAGS_BLOCK" "$SIGNED_FLAGS_BLOCK" | grep -Eq '^    var (a|b|c|d|e|delta|enabled):'; then
    echo "FAIL: import-c should not expand C bitfields as full-width struct fields"
    exit 1
fi

if [ "$(grep -Fxc 'fun Flags_get_a(recordPtr: *Flags) r: Int {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit getter helpers for unsigned bitfields"
    exit 1
fi

if [ "$(grep -Fxc 'fun Flags_set_e(recordPtr: *Flags, value: Int) r: Void {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit setter helpers for wide unsigned bitfields"
    exit 1
fi

if [ "$(grep -Fxc 'fun SignedFlags_get_delta(recordPtr: *SignedFlags) r: Int {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit signed getter helpers for bitfields"
    exit 1
fi

if [ "$(grep -Fxc 'fun SignedFlags_set_enabled(recordPtr: *SignedFlags, value: Int) r: Void {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit setter helpers for mixed signed/unsigned bitfield groups"
    exit 1
fi

if [ "$(grep -Fxc 'extern fun use_flags(arg0: *Flags, arg1: *SignedFlags) r: Void' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve typed pointers for bitfield-backed records"
    exit 1
fi

cat >>"$OUT" <<'EOF'

fun main() r: Int {
    let flagsAddr = ptr_alloc_i32()
    ptr_store_i32(flagsAddr, 0 as Int32)
    let flags = flagsAddr as *Flags
    Flags_set_a(flags, 1)
    Flags_set_b(flags, 5)
    Flags_set_c(flags, 12)
    Flags_set_d(flags, 200)
    Flags_set_e(flags, 65535)

    let flagsA = Flags_get_a(flags)
    if flagsA != 1 { return 1 }
    let flagsB = Flags_get_b(flags)
    if flagsB != 5 { return 2 }
    let flagsC = Flags_get_c(flags)
    if flagsC != 12 { return 3 }
    let flagsD = Flags_get_d(flags)
    if flagsD != 200 { return 4 }
    let flagsE = Flags_get_e(flags)
    if flagsE != 65535 { return 5 }

    let signedFlagsAddr = ptr_alloc_i32()
    ptr_store_i32(signedFlagsAddr, 0 as Int32)
    let signedFlags = signedFlagsAddr as *SignedFlags
    SignedFlags_set_delta(signedFlags, -3)
    SignedFlags_set_enabled(signedFlags, 1)

    let delta = SignedFlags_get_delta(signedFlags)
    if delta != -3 { return 6 }
    let enabled = SignedFlags_get_enabled(signedFlags)
    if enabled != 1 { return 7 }

    return 0
}
EOF

"$COMPILER" compile "$OUT" "$TMP_DIR/bitfields_probe" --fast >/dev/null
"$TMP_DIR/bitfields_probe" >/dev/null

echo "PASS: import-c emits ABI-correct bitfield storage with helper accessors"
