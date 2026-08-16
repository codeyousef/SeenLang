#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SEEN_BIN="${SEEN_BIN:-$ROOT/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT/scripts/run_capped_regression.sh"
SCOPE=seen-checked-allocation-codegen
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$SEEN_BIN" -- \
        bash "$0" "$@"
fi
SEEN_BIN="${SEEN_CAPPED_REGRESSION_COMPILER:-$SEEN_BIN}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$SEEN_BIN"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-checked-allocation-codegen.XXXXXX")"
SRC="$TMP_DIR/checked_alloc_regression.seen"
IR_DIR="$TMP_DIR/ir"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-checked-allocation-codegen.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) return 1 ;;
    esac
}
trap cleanup EXIT

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

bash "$ATTESTED_SEEN" "$SEEN_BIN" compile \
    --emit-module-ir-dir "$IR_DIR" --stop-after-ir "$SRC" >/dev/null

grep -R -q 'call ptr @seen_checked_malloc' "$IR_DIR"
! grep -R -q 'call ptr @malloc' "$IR_DIR"
! grep -R -q 'call noalias ptr @malloc' "$IR_DIR"

echo "PASS: compiler-emitted heap allocation uses Seen checked allocation"
