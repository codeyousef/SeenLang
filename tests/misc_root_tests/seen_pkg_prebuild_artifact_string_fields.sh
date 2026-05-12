#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_pkg_prebuild_strings.XXXXXX)"

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
        if [ "$CAP_KB" -ge 10485760 ]; then
            ulimit -v "$CAP_KB"
        fi
    fi
fi

mkdir -p "$TMP_DIR/dep/src" "$TMP_DIR/editor/src"

cat >"$TMP_DIR/dep/Seen.toml" <<'EOF'
[project]
name = "depstrings"
version = "0.1.0"
language = "en"
modules = ["src/meta.seen"]
EOF

cat >"$TMP_DIR/dep/src/spec.seen" <<'EOF'
pub class PropertyQuerySpec {
    var filterSearchText: String

    pub static fun new(text: String) r: PropertyQuerySpec {
        return PropertyQuerySpec { filterSearchText: text }
    }
}
EOF

cat >"$TMP_DIR/dep/src/meta.seen" <<'EOF'
import depstrings.spec.{PropertyQuerySpec}

pub fun makeSpec(text: String) r: PropertyQuerySpec {
    return PropertyQuerySpec.new(text)
}
EOF

"$COMPILER" pkg prebuild "$TMP_DIR/dep" "$TMP_DIR/dep_artifact" >/dev/null

grep -F "src/spec.seen" "$TMP_DIR/dep_artifact/objects.tsv" >/dev/null
if grep -F "module	src/spec.seen" "$TMP_DIR/dep_artifact/interface.index.tsv" >/dev/null; then
    echo "FAIL: test package should rely on artifact import discovery, not interface.index.tsv listing spec.seen"
    exit 1
fi

cat >"$TMP_DIR/editor/Seen.toml" <<EOF
[project]
name = "editorstrings"
version = "0.1.0"
language = "en"
modules = ["src/editor.seen"]

[dependencies]
depstrings = { artifact = "$TMP_DIR/dep_artifact" }
EOF

cat >"$TMP_DIR/editor/src/editor.seen" <<'EOF'
import depstrings.meta.{makeSpec}

pub fun fieldFilterText() r: String {
    let spec = makeSpec("needle")
    let search: String = spec.filterSearchText
    if search != "" {
        return search
    }
    return "fallback"
}

pub fun packageBoundaryStringProbe() r: Int {
    if fieldFilterText() != "needle" {
        return 1
    }
    return 0
}
EOF

"$COMPILER" pkg prebuild "$TMP_DIR/editor" "$TMP_DIR/editor_artifact" >/dev/null

grep -F "function	src/editor.seen	public	fieldFilterText" "$TMP_DIR/editor_artifact/interface.index.tsv" >/dev/null
grep -F "function	src/editor.seen	public	packageBoundaryStringProbe" "$TMP_DIR/editor_artifact/interface.index.tsv" >/dev/null

echo "PASS: prebuilt package artifacts preserve dependency String field access"
