#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SEEN_BIN="${SEEN_BIN:-$ROOT/compiler_seen/target/seen}"
TMP_DIR="${TMPDIR:-/tmp}/seen_checked_allocation_codegen"
SRC="$TMP_DIR/checked_alloc_regression.seen"
IR_DIR="$TMP_DIR/ir"

rm -rf "$TMP_DIR"
mkdir -p "$IR_DIR"

cat > "$SRC" <<'SEEN'
class WideBox {
    var a0: Int
    var a1: Int
    var a2: Int
    var a3: Int
    var a4: Int
    var a5: Int
    var a6: Int
    var a7: Int
    var a8: Int
    var a9: Int
    var a10: Int
    var a11: Int

    static fun new() r: WideBox {
        return WideBox {
            a0: 0, a1: 1, a2: 2, a3: 3,
            a4: 4, a5: 5, a6: 6, a7: 7,
            a8: 8, a9: 9, a10: 10, a11: 11
        }
    }
}

fun main() r: Int {
    let box = WideBox.new()
    if box.a11 == 11 {
        return 0
    }
    return 1
}
SEEN

"$SEEN_BIN" compile --emit-module-ir-dir "$IR_DIR" --stop-after-ir "$SRC" >/dev/null

grep -R -q 'call ptr @seen_checked_malloc' "$IR_DIR"
! grep -R -q 'call ptr @malloc' "$IR_DIR"
! grep -R -q 'call noalias ptr @malloc' "$IR_DIR"

echo "PASS: compiler-emitted heap allocation uses Seen checked allocation"
