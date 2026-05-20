#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_unresolved_struct_literal.XXXXXX)"

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

cat >"$TMP_DIR/missing_struct_arg.seen" <<'EOF'
fun acceptVertex(value: Int) {
}

fun main() {
    acceptVertex(MeshVertex {
        posX: 1.0,
        posY: 2.0
    })
}
EOF

LOG="$TMP_DIR/compile.log"
if "$COMPILER" compile "$TMP_DIR/missing_struct_arg.seen" "$TMP_DIR/out" \
    --no-cache --no-fork >"$LOG" 2>&1; then
    echo "FAIL: unresolved struct literal compiled successfully"
    cat "$LOG"
    exit 1
fi

if ! grep -F 'unresolved type `MeshVertex`' "$LOG" >/dev/null; then
    echo "FAIL: unresolved struct literal did not report a Seen diagnostic"
    cat "$LOG"
    exit 1
fi

if grep -F "inferred layout for unknown struct type" "$LOG" >/dev/null ||
    grep -F "/usr/bin/opt:" "$LOG" >/dev/null ||
    grep -F "/usr/bin/llc:" "$LOG" >/dev/null; then

    echo "FAIL: unresolved struct literal reached invalid LLVM IR"
    cat "$LOG"
    exit 1
fi

echo "PASS: unresolved struct literals fail before LLVM IR emission"
