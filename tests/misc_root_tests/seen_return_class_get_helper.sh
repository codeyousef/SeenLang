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

# Builtin generic names retain precedence. Keep this ordinary-class ABI fixture
# distinct from Box<T> rather than depending on an invalid unqualified Box.
cat >"$SRC" <<'EOF'
class ReturnedRecord {
    var name: String
    var count: Int

    static fun new(name: String, count: Int) r: ReturnedRecord {
        return ReturnedRecord {
            name: name,
            count: count
        }
    }
}

class BoxStore {
    var boxes: Array<ReturnedRecord>
    var count: Int

    static fun new() r: BoxStore {
        return BoxStore {
            boxes: Array<ReturnedRecord>(),
            count: 0
        }
    }

    fun put(box: ReturnedRecord) {
        this.boxes.push(box)
        this.count = this.count + 1
    }

    fun findIndex(name: String) r: Int {
        for i in 0..this.count {
            if this.boxes.get(i).name == name { return i }
        }
        return -1
    }

    fun get(name: String) r: ReturnedRecord {
        let idx = this.findIndex(name)
        if idx >= 0 { return this.boxes.get(idx) }
        return ReturnedRecord.new("", 0)
    }
}

fun main() r: Int {
    let store = BoxStore.new()
    store.put(ReturnedRecord.new("one", 1))
    store.put(ReturnedRecord.new("two", 2))

    let box = store.get("two")
    if box.count != 2 { return 1 }

    let direct = store.boxes.get(store.findIndex("two"))
    if direct.count != 2 { return 2 }

    return 0
}
EOF

for profile in fast release; do
    BIN="$TMP_DIR/return_class_get-$profile"
    LOG="$TMP_DIR/$profile.compile.log"
    OUT="$TMP_DIR/$profile.run.out"
    flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    if ! bash "$ATTESTED_SEEN" "$COMPILER" compile "$SRC" "$BIN" \
        --no-cache --target-cpu=x86-64 "${flags[@]}" >"$LOG" 2>&1; then
        echo "FAIL: class-return get helper repro did not compile ($profile)"
        cat "$LOG"
        exit 1
    fi
    bash "$ROOT_DIR/scripts/check_x86_executable_baseline.sh" x86-64 "$BIN"
    run_status=0
    timeout --foreground --kill-after=5s 120s "$BIN" >"$OUT" 2>&1 || run_status=$?
    if [ "$run_status" -ne 0 ]; then
        echo "FAIL: class-return get helper repro crashed ($profile)"
        echo "status=$run_status"
        cat "$OUT"
        exit 1
    fi
done

echo "PASS: class values returned from Array-backed get helpers remain usable"
