#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-build-modules-struct-arg
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-build-modules-struct-arg.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-build-modules-struct-arg.*)
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
bash "$ATTESTED_SEEN" "$COMPILER" compile "$TMP_DIR/main.seen" "$TMP_DIR/out" \
    --no-cache >"$LOG" 2>&1

if grep -F "Unknown struct type 'MeshVertex'" "$LOG" >/dev/null; then
    echo "FAIL: build.modules struct argument repro lost MeshVertex layout"
    cat "$LOG"
    exit 1
fi

"$TMP_DIR/out" >/dev/null

echo "PASS: build.modules preserves cross-module struct argument layouts"
