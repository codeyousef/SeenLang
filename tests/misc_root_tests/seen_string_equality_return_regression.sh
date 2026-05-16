#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_string_equality_return.XXXXXX)"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        rm -rf "$TMP_DIR"
    else
        echo "KEEP: $TMP_DIR"
    fi
}

trap cleanup EXIT

if [ -z "${SEEN_TEST_NO_ULIMIT:-}" ]; then
    AVAIL_KB="$(awk '/MemAvailable/ {print $2}' /proc/meminfo 2>/dev/null || echo 0)"
    if [ "$AVAIL_KB" -gt 0 ]; then
        CAP_KB=$(( AVAIL_KB * 70 / 100 ))
        if [ "$CAP_KB" -gt 14680064 ]; then
            CAP_KB=14680064
        fi
        if [ "$CAP_KB" -ge 8388608 ]; then
            ulimit -v "$CAP_KB"
        fi
    fi
fi

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

"$COMPILER" run --aot "$TMP_DIR/direct_literal.seen" >/dev/null
"$COMPILER" run --aot "$TMP_DIR/direct_concat.seen" >/dev/null
"$COMPILER" run --aot "$TMP_DIR/imported/main.seen" >/dev/null

INVALID_LOG="$TMP_DIR/invalid.log"
if "$COMPILER" compile "$TMP_DIR/invalid_mismatch.seen" "$TMP_DIR/invalid_out" \
    --no-cache --no-fork >"$INVALID_LOG" 2>&1; then
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
