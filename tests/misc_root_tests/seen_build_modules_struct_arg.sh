#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_build_modules_struct_arg.XXXXXX)"

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
name = "meshdata_struct_arg_repro"
version = "0.1.0"
language = "en"
description = "Regression for build.modules struct argument discovery"

[build]
entry = "main.seen"
modules = [
    "greedy_data.seen",
    "main.seen"
]
EOF

cat >"$TMP_DIR/greedy_data.seen" <<'EOF'
class MeshVertex {
    var posX: Float
    var posY: Float
    var posZ: Float
    var normalX: Float
    var normalY: Float
    var normalZ: Float
    var u: Float
    var v: Float
    var voxelType: Float
    var ao: Float
}

class MeshData {
    var vertices: Array<Float>
    var vertexCount: Int

    static fun new() r: MeshData {
        return MeshData {
            vertices: Array<Float>(),
            vertexCount: 0
        }
    }

    fun addVertex(vertex: MeshVertex) {
        this.vertices.push(vertex.posX)
        this.vertices.push(vertex.posY)
        this.vertices.push(vertex.posZ)
        this.vertices.push(vertex.normalX)
        this.vertices.push(vertex.normalY)
        this.vertices.push(vertex.normalZ)
        this.vertices.push(vertex.u)
        this.vertices.push(vertex.v)
        this.vertices.push(vertex.voxelType)
        this.vertices.push(vertex.ao)
        this.vertexCount = this.vertexCount + 1
    }
}
EOF

cat >"$TMP_DIR/main.seen" <<'EOF'
fun main() {
    let mesh = MeshData.new()
    mesh.addVertex(MeshVertex {
        posX: 1.0,
        posY: 2.0,
        posZ: 3.0,
        normalX: 0.0,
        normalY: 1.0,
        normalZ: 0.0,
        u: 0.5,
        v: 0.25,
        voxelType: 2.0,
        ao: 1.0
    })
}
EOF

LOG="$TMP_DIR/compile.log"
"$COMPILER" compile "$TMP_DIR/main.seen" "$TMP_DIR/out" \
    --no-cache --no-fork >"$LOG" 2>&1

if grep -F "Unknown struct type 'MeshVertex'" "$LOG" >/dev/null; then
    echo "FAIL: build.modules struct argument repro lost MeshVertex layout"
    cat "$LOG"
    exit 1
fi

"$TMP_DIR/out" >/dev/null

echo "PASS: build.modules preserves cross-module struct argument layouts"
