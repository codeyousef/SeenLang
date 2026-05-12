#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

cat > "$TMP_DIR/input.ll" <<'IR_EOF'
target triple = "x86_64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

%SeenString = type { i64, ptr }

@.str = private unnamed_addr constant [1 x i8] c"\00"

define %SeenString @roundtrip(%SeenString %value.arg) {
entry:
  %slot = alloca %SeenString
  store %SeenString %value.arg, ptr %slot
  %loaded = load %SeenString, ptr %slot
  ret %SeenString %loaded
}

define void @caller() {
entry:
  %ptr = getelementptr [1 x i8], ptr @.str, i64 0, i64 0
  %a = insertvalue %SeenString undef, i64 0, 0
  %b = insertvalue %SeenString %a, ptr %ptr, 1
  %c = call %SeenString @roundtrip(%SeenString %b)
  ret void
}
IR_EOF

python3 "$ROOT_DIR/scripts/ll_win64_abi.py" "$TMP_DIR/input.ll" "$TMP_DIR/output.ll" >/dev/null

rg -q 'define void @roundtrip\(ptr sret\(%SeenString\) %_sret_out, ptr byval\(%SeenString\) %value.arg.byval\)' "$TMP_DIR/output.ll"
rg -q '  %value.arg = load %SeenString, ptr %value.arg.byval' "$TMP_DIR/output.ll"
rg -q 'store %SeenString %value.arg, ptr %slot' "$TMP_DIR/output.ll"

LLC="$(command -v llc-20 2>/dev/null || command -v llc 2>/dev/null || true)"
if [[ -n "$LLC" ]]; then
    "$LLC" "$TMP_DIR/output.ll" -o "$TMP_DIR/output.s" -mtriple=x86_64-w64-mingw32 -O0 --filetype=asm
fi

echo "windows ABI transform byval parameter test passed"
