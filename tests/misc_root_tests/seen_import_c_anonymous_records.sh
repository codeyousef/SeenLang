#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_DIR="$(mktemp -d /tmp/seen_import_c_anonymous_records.XXXXXX)"
HEADER="$TMP_DIR/anonymous_records.h"
OUT="$TMP_DIR/anonymous_records_bindings.seen"

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT

cat >"$HEADER" <<'EOF'
typedef struct Outer {
    int tag;
    union {
        int raw;
        float f;
    } data;
    struct {
        int width;
        int height;
    };
} Outer;

void use_outer(Outer *outer);
EOF

"$COMPILER" import-c "$HEADER" | sed -n '/^\/\/ Auto-generated/,$p' >"$OUT"

if [ "$(grep -Fxc 'class Outer_anon_union_0 {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should synthesize a named union class for anonymous union members"
    exit 1
fi

if [ "$(grep -Fxc 'class Outer_anon_struct_1 {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should synthesize a named struct class for anonymous struct members"
    exit 1
fi

ANON_UNION_BLOCK="$(sed -n '/^class Outer_anon_union_0 {$/,/^}$/p' "$OUT")"

if ! printf '%s\n' "$ANON_UNION_BLOCK" | grep -Fqx '    var raw: Int32'; then
    echo "FAIL: import-c should preserve fields inside synthesized anonymous unions"
    exit 1
fi

if ! printf '%s\n' "$ANON_UNION_BLOCK" | grep -Fqx '    var f: Float32'; then
    echo "FAIL: import-c should preserve floating fields inside synthesized anonymous unions"
    exit 1
fi

ANON_STRUCT_BLOCK="$(sed -n '/^class Outer_anon_struct_1 {$/,/^}$/p' "$OUT")"

if ! printf '%s\n' "$ANON_STRUCT_BLOCK" | grep -Fqx '    var width: Int32'; then
    echo "FAIL: import-c should preserve fields inside synthesized anonymous structs"
    exit 1
fi

if ! printf '%s\n' "$ANON_STRUCT_BLOCK" | grep -Fqx '    var height: Int32'; then
    echo "FAIL: import-c should preserve trailing fields inside synthesized anonymous structs"
    exit 1
fi

OUTER_BLOCK="$(sed -n '/^class Outer {$/,/^}$/p' "$OUT")"

if ! printf '%s\n' "$OUTER_BLOCK" | grep -Fqx '    var tag: Int32'; then
    echo "FAIL: import-c should preserve leading fields in parent records with anonymous members"
    exit 1
fi

if ! printf '%s\n' "$OUTER_BLOCK" | grep -Fqx '    var data: Outer_anon_union_0'; then
    echo "FAIL: import-c should reuse synthesized anonymous union types in parent fields"
    exit 1
fi

if ! printf '%s\n' "$OUTER_BLOCK" | grep -Fqx '    var anon_struct_0: Outer_anon_struct_1'; then
    echo "FAIL: import-c should synthesize parent fields for fully anonymous struct members"
    exit 1
fi

if printf '%s\n' "$OUTER_BLOCK" | grep -Eq '    var (raw|f|width|height):'; then
    echo "FAIL: import-c should not flatten anonymous member aliases directly into the parent record"
    exit 1
fi

if [ "$(grep -Fxc 'extern fun use_outer(arg0: *Outer) r: Void' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve parent record pointers when anonymous members are present"
    exit 1
fi

cat >>"$OUT" <<'EOF'

fun main() r: Void {
}
EOF

"$COMPILER" compile "$OUT" "$TMP_DIR/anonymous_records_probe" --fast >/dev/null

echo "PASS: import-c emits synthesized anonymous record layouts"
