#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-string-equality-return
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-string-equality-return.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-string-equality-return.*)
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

cat >"$TMP_DIR/direct_literal.seen" <<'EOF'
fun setWorldSaveRoot(root: String) r: Void {
}

fun worldSaveDir(seed: Int) r: String {
    return "saves/world_42_v4"
}

fun main() r: Int {
    setWorldSaveRoot("")
    if (worldSaveDir(42) == "saves/world_42_v4") as Int {
        return 0
    }
    return 1
}
EOF

cat >"$TMP_DIR/direct_concat.seen" <<'EOF'
fun setWorldSaveRoot(root: String) r: Void {
}

fun worldSaveDir(seed: Int) r: String {
    return "saves/world_" + seed.toString() + "_v4"
}

fun main() r: Int {
    setWorldSaveRoot("")
    if (worldSaveDir(42) == "saves/world_42_v4") as Int {
        return 0
    }
    return 1
}
EOF

mkdir -p "$TMP_DIR/imported"
cat >"$TMP_DIR/imported/chunk_store.seen" <<'EOF'
pub fun setWorldSaveRoot(root: String) r: Void {
}

pub fun worldSaveDir(seed: Int) r: String {
    return "saves/world_" + seed.toString() + "_v4"
}
EOF

cat >"$TMP_DIR/imported/main.seen" <<'EOF'
import chunk_store.{setWorldSaveRoot, worldSaveDir}

fun main() r: Int {
    setWorldSaveRoot("")
    if (worldSaveDir(42) == "saves/world_42_v4") as Int {
        return 0
    }
    return 1
}
EOF

cat >"$TMP_DIR/invalid_mismatch.seen" <<'EOF'
fun main() r: Int {
    return ((42 == "saves/world_42_v4") as Int)
}
EOF

bash "$ATTESTED_SEEN" "$COMPILER" run --aot "$TMP_DIR/direct_literal.seen" >/dev/null
bash "$ATTESTED_SEEN" "$COMPILER" run --aot "$TMP_DIR/direct_concat.seen" >/dev/null
bash "$ATTESTED_SEEN" "$COMPILER" run --aot "$TMP_DIR/imported/main.seen" >/dev/null

INVALID_LOG="$TMP_DIR/invalid.log"
if bash "$ATTESTED_SEEN" "$COMPILER" compile \
    "$TMP_DIR/invalid_mismatch.seen" "$TMP_DIR/invalid_out" \
    --no-cache >"$INVALID_LOG" 2>&1; then
    echo "FAIL: mismatched String equality compiled successfully"
    cat "$INVALID_LOG"
    exit 1
fi

if ! grep -F "cannot compare" "$INVALID_LOG" >/dev/null; then
    echo "FAIL: mismatched String equality did not report a Seen diagnostic"
    cat "$INVALID_LOG"
    exit 1
fi

if grep -F "seen_str_eq_ss(%SeenString 42" "$INVALID_LOG" >/dev/null ||
    grep -F "/usr/bin/opt:" "$INVALID_LOG" >/dev/null ||
    grep -F "/usr/bin/llc:" "$INVALID_LOG" >/dev/null; then

    echo "FAIL: mismatched String equality reached invalid LLVM IR"
    cat "$INVALID_LOG"
    exit 1
fi

echo "PASS: direct String return equality lowers safely"
