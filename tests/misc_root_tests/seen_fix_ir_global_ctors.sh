#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_fix_ir_global_ctors.XXXXXX)"
LL_FILE="$TMP_DIR/test.ll"

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT

cat >"$LL_FILE" <<'EOF'
source_filename = "test"
target triple = "x86_64-unknown-linux-gnu"

@llvm.global_ctors = appending global [1 x { i32, ptr, ptr }] [
  { i32, ptr, ptr } { i32 65535, ptr @__seen_internal_module_init__, ptr null }
]

define void @__seen_internal_module_init__() {
entry:
  ret void
}

define i32 @main() {
entry:
  call void @exit(i64 1)
  ret i32 0
}
EOF

python3 "$ROOT_DIR/scripts/fix_ir.py" "$LL_FILE" >/dev/null

EXIT_LINE="$(rg -n '^declare void @exit\(i64\) nounwind$' "$LL_FILE" | cut -d: -f1)"
CTOR_LINE="$(rg -n '^@llvm\.global_ctors = ' "$LL_FILE" | cut -d: -f1)"
CTOR_END_LINE="$(awk -v start="$CTOR_LINE" 'NR > start && /^\]$/ { print NR; exit }' "$LL_FILE")"

if [ -z "$EXIT_LINE" ] || [ -z "$CTOR_LINE" ] || [ -z "$CTOR_END_LINE" ]; then
    echo "FAIL: expected declaration or ctor block missing after fix_ir.py"
    cat "$LL_FILE"
    exit 1
fi

if [ "$EXIT_LINE" -gt "$CTOR_LINE" ] && [ "$EXIT_LINE" -lt "$CTOR_END_LINE" ]; then
    echo "FAIL: fix_ir.py inserted @exit inside @llvm.global_ctors"
    cat "$LL_FILE"
    exit 1
fi

llvm-as "$LL_FILE" -o /dev/null

echo "PASS: fix_ir.py keeps synthesized declarations outside @llvm.global_ctors"
