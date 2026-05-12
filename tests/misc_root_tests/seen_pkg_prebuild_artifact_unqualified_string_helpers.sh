#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_pkg_prebuild_unqualified_strings.XXXXXX)"

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

mkdir -p "$TMP_DIR/facade/src" "$TMP_DIR/consumer/src"

cat >"$TMP_DIR/facade/Seen.toml" <<'EOF'
[project]
name = "facade_repro"
version = "0.1.0"
language = "en"
modules = ["src/string_helpers.seen"]
EOF

cat >"$TMP_DIR/facade/src/string_helpers.seen" <<'EOF'
fun reproAppendText(base: String, value: String) r: String {
    if value == "" {
        return base
    }
    if base == "" {
        return value
    }
    return base + "," + value
}

fun reproListText(values: Array<String>) r: String {
    var text = ""
    for i in 0..values.length() {
        text = reproAppendText(text, values.get(i))
    }
    return text
}
EOF

"$COMPILER" pkg prebuild "$TMP_DIR/facade" "$TMP_DIR/build/facade" >/dev/null

cat >"$TMP_DIR/consumer/Seen.toml" <<EOF
[project]
name = "consumer_repro"
version = "0.1.0"
language = "en"
modules = ["src/use_dependency.seen"]

[dependencies]
facade_repro = { artifact = "$TMP_DIR/build/facade" }
EOF

cat >"$TMP_DIR/consumer/src/use_dependency.seen" <<'EOF'
fun reproConsumerJoin(values: Array<String>) r: String {
    var text = ""
    for i in 0..values.length() {
        text = reproAppendText(text, values.get(i))
    }
    return text
}
EOF

"$COMPILER" pkg prebuild "$TMP_DIR/consumer" "$TMP_DIR/build/consumer" >/dev/null

grep -F "function	src/use_dependency.seen	package	reproConsumerJoin" \
    "$TMP_DIR/build/consumer/interface.index.tsv" >/dev/null

echo "PASS: prebuilt package artifacts preserve unqualified dependency String helper ABI"
