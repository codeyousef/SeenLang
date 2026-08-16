#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-return-class-get-helper
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-return-class-get-helper.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-return-class-get-helper.*)
                [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                    [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
                rm -rf -- "$TMP_DIR"
                ;;
            *) return 1 ;;
        esac
    else
        echo "KEEP: $TMP_DIR"
    fi
}

trap cleanup EXIT

SRC="$TMP_DIR/return_class_get.seen"
BIN="$TMP_DIR/return_class_get"
LOG="$TMP_DIR/compile.log"
OUT="$TMP_DIR/run.out"

cat >"$SRC" <<'EOF'
class Box {
    var name: String
    var count: Int

    static fun new(name: String, count: Int) r: Box {
        return Box {
            name: name,
            count: count
        }
    }
}

class BoxStore {
    var boxes: Array<Box>
    var count: Int

    static fun new() r: BoxStore {
        return BoxStore {
            boxes: Array<Box>(),
            count: 0
        }
    }

    fun put(box: Box) {
        this.boxes.push(box)
        this.count = this.count + 1
    }

    fun findIndex(name: String) r: Int {
        for i in 0..this.count {
            if this.boxes.get(i).name == name { return i }
        }
        return -1
    }

    fun get(name: String) r: Box {
        let idx = this.findIndex(name)
        if idx >= 0 { return this.boxes.get(idx) }
        return Box.new("", 0)
    }
}

fun main() r: Int {
    let store = BoxStore.new()
    store.put(Box.new("one", 1))
    store.put(Box.new("two", 2))

    let box = store.get("two")
    if box.count != 2 { return 1 }

    let direct = store.boxes.get(store.findIndex("two"))
    if direct.count != 2 { return 2 }

    return 0
}
EOF

if ! bash "$ATTESTED_SEEN" "$COMPILER" compile "$SRC" "$BIN" \
    --fast --no-cache >"$LOG" 2>&1; then
    echo "FAIL: class-return get helper repro did not compile"
    cat "$LOG"
    exit 1
fi

run_status=0
"$BIN" >"$OUT" 2>&1 || run_status=$?
if [ "$run_status" -ne 0 ]; then
    echo "FAIL: class-return get helper repro crashed"
    echo "status=$run_status"
    cat "$OUT"
    exit 1
fi

echo "PASS: class values returned from Array-backed get helpers remain usable"
