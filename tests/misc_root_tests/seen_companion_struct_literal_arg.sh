#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_companion_struct_literal_arg.XXXXXX)"

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

cat >"$TMP_DIR/Seen.toml" <<'EOF'
[project]
name = "companion_struct_literal_arg"
version = "0.1.0"
language = "en"
modules = [
    "greedy.seen",
    "main.seen"
]

[build]
entry = "main.seen"
EOF

cat >"$TMP_DIR/greedy.seen" <<'EOF'
fun makeMesh() r: MeshData {
    return MeshData.new()
}
EOF

cat >"$TMP_DIR/greedy_data.seen" <<'EOF'
class MeshVertex {
    var x: Float
}

class MeshData {
    var count: Int

    static fun new() r: MeshData {
        return MeshData { count: 0 }
    }

    fun addVertex(vertex: MeshVertex) {
        this.count = this.count + 1
    }
}
EOF

cat >"$TMP_DIR/main.seen" <<'EOF'
import greedy

fun main() r: Int {
    let mesh = makeMesh()
    mesh.addVertex(MeshVertex { x: 1.0 })
    if mesh.count == 1 {
        return 0
    }
    return 1
}
EOF

LOG="$TMP_DIR/compile.log"
"$COMPILER" compile "$TMP_DIR/main.seen" "$TMP_DIR/out" \
    --no-cache --no-fork >"$LOG" 2>&1

if grep -E 'unresolved type .*MeshVertex|undefined symbol: MeshData_|defined with type .ptr. but expected .i64.' "$LOG" >/dev/null; then
    echo "FAIL: companion struct-literal method argument regression"
    cat "$LOG"
    exit 1
fi

"$TMP_DIR/out" >/dev/null

echo "PASS: companion struct-literal method argument regression"
