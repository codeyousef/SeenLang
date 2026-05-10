#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_pkg_prebuild_symbols.XXXXXX)"

cleanup() {
    rm -rf "$TMP_DIR"
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

mkdir -p "$TMP_DIR/pkg/src" "$TMP_DIR/consumer/src"

cat >"$TMP_DIR/pkg/Seen.toml" <<'EOF'
[project]
name = "libx"
version = "0.1.0"
language = "en"
modules = ["src/value.seen"]
EOF

cat >"$TMP_DIR/pkg/src/value.seen" <<'EOF'
pub fun libx_value() r: Int {
    return 7
}

@export
pub fun libx_exported_value() r: Int {
    return 8
}
EOF

"$COMPILER" pkg prebuild "$TMP_DIR/pkg" "$TMP_DIR/artifact" >/dev/null

OBJ="$TMP_DIR/artifact/$(awk -F '	' '$2 ~ /value\.seen$/ {print $1}' "$TMP_DIR/artifact/objects.tsv")"
if [ ! -f "$OBJ" ]; then
    echo "FAIL: prebuild object for value.seen was not created"
    exit 1
fi

if command -v llvm-nm >/dev/null 2>&1; then
    NM_TOOL=llvm-nm
else
    NM_TOOL=nm
fi

mkdir -p "$TMP_DIR/local/bin"
cp "$COMPILER" "$TMP_DIR/local/bin/seen"
chmod +x "$TMP_DIR/local/bin/seen"
CWD_COMPILER="$TMP_DIR/local/bin/seen"

mkdir -p "$TMP_DIR/cwd_export/src"
cat >"$TMP_DIR/cwd_export/Seen.toml" <<'EOF'
[project]
name = "cwd_export"
version = "0.1.0"
language = "en"
modules = ["src/hot_module.seen"]
EOF

cat >"$TMP_DIR/cwd_export/.entry.seen" <<'EOF'
import src::hot_module

fun main() r: Int {
    return 0
}
EOF

cat >"$TMP_DIR/cwd_export/src/hot_module.seen" <<'EOF'
let FEL205_HOT_API_VERSION = 205

@export
fun fel205_export_probe() r: Int {
    return FEL205_HOT_API_VERSION
}
EOF

(
    cd "$TMP_DIR/cwd_export"
    "$CWD_COMPILER" compile "$TMP_DIR/cwd_export/.entry.seen" "$TMP_DIR/cwd_export/out" --pic --no-fork --no-cache --object-manifest "$TMP_DIR/cwd_export/objects.tsv" --emit-llvm >/dev/null
)

CWD_OBJ_REL="$(awk -F '	' '$2 ~ /hot_module\.seen$/ {print $1}' "$TMP_DIR/cwd_export/objects.tsv")"
CWD_OBJ="$CWD_OBJ_REL"
if [[ "$CWD_OBJ_REL" != /* ]]; then
    CWD_OBJ="$TMP_DIR/cwd_export/$CWD_OBJ_REL"
fi
if [ ! -f "$CWD_OBJ" ]; then
    echo "FAIL: cwd import object for hot_module.seen was not created"
    exit 1
fi

"$NM_TOOL" --defined-only "$CWD_OBJ" | grep -E 'FEL205_HOT_API_VERSION|fel205_export_probe' >/dev/null || {
    echo "FAIL: cwd import object does not define exported hot-module symbols"
    "$NM_TOOL" --defined-only "$CWD_OBJ" || true
    exit 1
}

LL_HIT="$(find "$TMP_DIR/cwd_export" -name 'out.module*.ll' -exec grep -El 'define .*fel205_export_probe|@FEL205_HOT_API_VERSION' {} + | head -n 1 || true)"
if [ -z "$LL_HIT" ]; then
    echo "FAIL: cwd import LLVM did not contain the imported exported declaration"
    exit 1
fi

"$NM_TOOL" --defined-only "$OBJ" | grep -E 'libx_value|libx_exported_value' >/dev/null || {
    echo "FAIL: prebuild object does not define package/export symbols"
    "$NM_TOOL" --defined-only "$OBJ" || true
    exit 1
}

grep -F "function	src/value.seen	public	libx_value" "$TMP_DIR/artifact/interface.index.tsv" >/dev/null
grep -F "function	src/value.seen	public	libx_exported_value" "$TMP_DIR/artifact/interface.index.tsv" >/dev/null

cat >"$TMP_DIR/consumer/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"
language = "en"

[dependencies]
libx = { artifact = "$TMP_DIR/artifact" }
EOF

cat >"$TMP_DIR/consumer/src/main.seen" <<'EOF'
import libx.value.{libx_value, libx_exported_value}

fun main() r: Int {
    if libx_value() != 7 {
        return 1
    }
    if libx_exported_value() != 8 {
        return 2
    }
    return 0
}
EOF

"$COMPILER" compile "$TMP_DIR/consumer/src/main.seen" "$TMP_DIR/consumer_bin" --fast >/dev/null
"$TMP_DIR/consumer_bin"

echo "PASS: prebuilt package artifacts preserve public and exported symbols for downstream consumers"
