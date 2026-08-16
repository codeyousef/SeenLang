#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-import-c-arrays
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-import-c-arrays.XXXXXX")"
HEADER="$TMP_DIR/arrays.h"
FULL_OUT="$TMP_DIR/arrays_full.out"
OUT="$TMP_DIR/arrays_bindings.seen"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-import-c-arrays.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR" ;;
        *) return 1 ;;
    esac
}

trap cleanup EXIT

cat >"$HEADER" <<'EOF'
typedef struct Pixel {
    float rgba[4];
} Pixel;

typedef union ClearValue {
    int values[4];
} ClearValue;

void use_arrays(Pixel *pixel, ClearValue *clear);
EOF

bash "$ATTESTED_SEEN" "$COMPILER" import-c "$HEADER" >"$FULL_OUT"
sed -n '/^\/\/ Auto-generated/,$p' "$FULL_OUT" >"$OUT"

if grep -Fq 'Warning:' "$FULL_OUT"; then
    echo "FAIL: import-c should not warn for supported inline C arrays"
    exit 1
fi

if [ "$(grep -Fxc 'class Pixel {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit a repr(C) class for array-backed structs"
    exit 1
fi

if [ "$(grep -Fxc 'class ClearValue {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit a union class for array-backed unions"
    exit 1
fi

if [ "$(grep -c '^    var rgba: Float32\[4\]$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit fixed-array struct fields"
    exit 1
fi

if [ "$(grep -c '^    var values: Int32\[4\]$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit fixed-array union fields"
    exit 1
fi

if [ "$(grep -Fxc 'extern fun use_arrays(arg0: *Pixel, arg1: *ClearValue) r: Void' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should keep typed pointers to array-backed imported records"
    exit 1
fi

cat >>"$OUT" <<'EOF'

fun main() r: Void {
}
EOF

bash "$ATTESTED_SEEN" "$COMPILER" compile \
    "$OUT" "$TMP_DIR/arrays_probe" --fast >/dev/null

echo "PASS: import-c emits inline array layouts for imported structs and unions"
