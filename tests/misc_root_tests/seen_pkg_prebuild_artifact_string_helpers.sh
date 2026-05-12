#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_pkg_prebuild_string_helpers.XXXXXX)"

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

mkdir -p "$TMP_DIR/facade/src" "$TMP_DIR/consumer/src"

cat >"$TMP_DIR/facade/Seen.toml" <<'EOF'
[project]
name = "facade"
version = "0.1.0"
language = "en"
modules = ["src/facade.seen"]

[build]
entry = "src/build_entry.seen"
EOF

cat >"$TMP_DIR/facade/src/facade.seen" <<'EOF'
// The public facade root intentionally omits the helper module. The helper is
// still part of the prebuilt object set via the package build entry.
EOF

cat >"$TMP_DIR/facade/src/build_entry.seen" <<'EOF'
import facade.inspector_model.{propertyInspectorAppendText}

fun facadeBuildProbe() r: String {
    return propertyInspectorAppendText("a", "b")
}
EOF

cat >"$TMP_DIR/facade/src/inspector_model.seen" <<'EOF'
@export
fun propertyInspectorAppendText(prefix: String, suffix: String) r: String {
    return prefix + suffix
}
EOF

"$COMPILER" pkg prebuild "$TMP_DIR/facade" "$TMP_DIR/facade_artifact" >/dev/null

grep -F "src/inspector_model.seen" "$TMP_DIR/facade_artifact/objects.tsv" >/dev/null
if grep -F "module	src/inspector_model.seen" "$TMP_DIR/facade_artifact/interface.index.tsv" >/dev/null; then
    echo "FAIL: test package should rely on object-manifest declaration discovery, not interface.index.tsv listing inspector_model.seen"
    exit 1
fi

cat >"$TMP_DIR/consumer/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"
language = "en"
modules = ["src/consumer.seen"]

[dependencies]
facade = { artifact = "$TMP_DIR/facade_artifact" }
EOF

cat >"$TMP_DIR/consumer/src/consumer.seen" <<'EOF'
import facade.{propertyInspectorAppendText}

pub fun packageBoundaryStringHelperProbe() r: String {
    let result: String = propertyInspectorAppendText("a", "b")
    return result
}
EOF

"$COMPILER" pkg prebuild "$TMP_DIR/consumer" "$TMP_DIR/consumer_artifact" >/dev/null

grep -F "function	src/consumer.seen	public	packageBoundaryStringHelperProbe" "$TMP_DIR/consumer_artifact/interface.index.tsv" >/dev/null

echo "PASS: prebuilt package artifacts preserve dependency String helper return types"
