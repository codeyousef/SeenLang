#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
SEEN_COMPILE_CMD="${SEEN_COMPILE_CMD:-compile}"
TMP_DIR="$(mktemp -d /tmp/seen_pkg_local_registry.XXXXXX)"
REGISTRY_DIR="$TMP_DIR/registry"
PACKAGE_DIR="$TMP_DIR/mathx"
CONSUMER_DIR="$TMP_DIR/consumer"
OUTPUT_BIN="$TMP_DIR/consumer_bin"

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT

mkdir -p "$REGISTRY_DIR" "$PACKAGE_DIR/src" "$CONSUMER_DIR/src"

cat >"$PACKAGE_DIR/Seen.toml" <<EOF
[project]
name = "mathx"
version = "0.1.0"

[dependencies]

[native.dependencies]
EOF

cat >"$PACKAGE_DIR/src/value.seen" <<'EOF'
fun answer() r: Int {
    return 42
}
EOF

(
    cd "$PACKAGE_DIR"
    "$COMPILER" pkg publish "$REGISTRY_DIR" >/dev/null
)

if [ ! -f "$REGISTRY_DIR/index/mathx.toml" ]; then
    echo "FAIL: pkg publish did not create package index entry"
    exit 1
fi

if [ ! -f "$REGISTRY_DIR/archives/mathx/mathx-0.1.0.seenpkg.tgz" ]; then
    echo "FAIL: pkg publish did not create package archive"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
mathx = "0.1.0"

[native.dependencies]
EOF

cat >"$CONSUMER_DIR/src/main.seen" <<'EOF'
import mathx.value.{answer}

fun main() r: Int {
    if answer() != 42 {
        return 1
    }
    return 0
}
EOF

"$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null

if [ ! -f "$CONSUMER_DIR/Seen.lock" ]; then
    echo "FAIL: pkg fetch did not write Seen.lock"
    exit 1
fi

if [ ! -f "$CONSUMER_DIR/.seen/packages/mathx/0.1.0/Seen.toml" ]; then
    echo "FAIL: pkg fetch did not install the package into .seen/packages"
    exit 1
fi

"$COMPILER" "$SEEN_COMPILE_CMD" "$CONSUMER_DIR/src/main.seen" "$OUTPUT_BIN" --fast >/dev/null
"$OUTPUT_BIN" >/dev/null

echo "PASS: seen pkg can publish to a local registry and consume the package from Seen.toml"
