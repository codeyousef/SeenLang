#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-companion-struct-literal-arg
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-companion-struct-literal-arg.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-companion-struct-literal-arg.*)
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
bash "$ATTESTED_SEEN" "$COMPILER" compile "$TMP_DIR/main.seen" "$TMP_DIR/out" \
    --no-cache >"$LOG" 2>&1

if grep -E 'unresolved type .*MeshVertex|undefined symbol: MeshData_|defined with type .ptr. but expected .i64.' "$LOG" >/dev/null; then
    echo "FAIL: companion struct-literal method argument regression"
    cat "$LOG"
    exit 1
fi

"$TMP_DIR/out" >/dev/null

echo "PASS: companion struct-literal method argument regression"
