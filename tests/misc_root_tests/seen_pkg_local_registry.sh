#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
SEEN_COMPILE_CMD="${SEEN_COMPILE_CMD:-compile}"
[ "$SEEN_COMPILE_CMD" = "compile" ] || {
    echo "FAIL: only the supported compile command is accepted" >&2
    exit 2
}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-pkg-local-registry
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-pkg-local-registry.XXXXXX")"
REGISTRY_DIR="$TMP_DIR/registry"
PACKAGE_DIR="$TMP_DIR/mathx"
CONSUMER_DIR="$TMP_DIR/consumer"
OUTPUT_BIN="$TMP_DIR/consumer_bin"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-pkg-local-registry.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) return 1 ;;
    esac
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
pub fun answer() r: Int {
    return 42
}
EOF

(
    cd "$PACKAGE_DIR"
    bash "$ATTESTED_SEEN" "$COMPILER" pkg publish "$REGISTRY_DIR" >/dev/null
)

if [ ! -f "$REGISTRY_DIR/index/mathx.toml" ]; then
    echo "FAIL: pkg publish did not create package index entry"
    exit 1
fi

if [ ! -f "$REGISTRY_DIR/archives/mathx/mathx-0.1.0.seenpkg.tgz" ]; then
    echo "FAIL: pkg publish did not create package archive"
    exit 1
fi

CHECKSUM_FAILURE_REGISTRY="$TMP_DIR/checksum-failure-registry"
CHECKSUM_FAILURE_BIN="$TMP_DIR/checksum-failure-bin"
CHECKSUM_FAILURE_OUTPUT="$TMP_DIR/checksum-failure.log"
mkdir -p "$CHECKSUM_FAILURE_REGISTRY" "$CHECKSUM_FAILURE_BIN"
cat >"$CHECKSUM_FAILURE_BIN/sha256sum" <<'EOF'
#!/usr/bin/env sh
exit 1
EOF
chmod +x "$CHECKSUM_FAILURE_BIN/sha256sum"
if (
    cd "$PACKAGE_DIR"
    SEEN_ATTESTED_SEEN_PATH="$SEEN_BOUNDED_TOOLCHAIN_DIR:$CHECKSUM_FAILURE_BIN:$PATH" \
        bash "$ATTESTED_SEEN" "$COMPILER" pkg publish \
        "$CHECKSUM_FAILURE_REGISTRY"
) >"$CHECKSUM_FAILURE_OUTPUT" 2>&1; then
    echo "FAIL: pkg publish accepted an archive without a sha256 digest"
    exit 1
fi
grep -Fq 'failed to compute archive sha256' "$CHECKSUM_FAILURE_OUTPUT"
if [ -e "$CHECKSUM_FAILURE_REGISTRY/archives/mathx/mathx-0.1.0.seenpkg.tgz" ]; then
    echo "FAIL: checksum failure left an orphaned publish archive"
    exit 1
fi
if [ -e "$CHECKSUM_FAILURE_REGISTRY/index/mathx.toml" ]; then
    echo "FAIL: checksum failure created a registry index entry"
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

bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null

if [ ! -f "$CONSUMER_DIR/Seen.lock" ]; then
    echo "FAIL: pkg fetch did not write Seen.lock"
    exit 1
fi

if [ "$(find "$CONSUMER_DIR/.seen/packages/mathx/0.1.0" \
    -mindepth 2 -maxdepth 2 -name Seen.toml | wc -l)" -ne 1 ]; then
    echo "FAIL: pkg fetch did not install the package into .seen/packages"
    exit 1
fi

bash "$ATTESTED_SEEN" "$COMPILER" "$SEEN_COMPILE_CMD" \
    "$CONSUMER_DIR/src/main.seen" "$OUTPUT_BIN" --fast >/dev/null
"$OUTPUT_BIN" >/dev/null

sed -i 's|^sha256 = .*|sha256 = "../../victim"|' \
    "$REGISTRY_DIR/index/mathx.toml"
mkdir -p "$CONSUMER_DIR/.seen/packages/victim"
cat >"$CONSUMER_DIR/.seen/packages/victim/Seen.toml" <<'EOF'
[project]
name = "victim"
version = "9.9.9"
EOF
touch "$CONSUMER_DIR/.seen/packages/victim/sentinel"
MALFORMED_DIGEST_OUTPUT="$TMP_DIR/malformed-digest.log"
if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" \
    >"$MALFORMED_DIGEST_OUTPUT" 2>&1; then
    echo "FAIL: malformed legacy digest was accepted as a cache path"
    exit 1
fi
grep -Fq 'has an invalid sha256 digest' "$MALFORMED_DIGEST_OUTPUT"
if [ ! -f "$CONSUMER_DIR/.seen/packages/victim/sentinel" ]; then
    echo "FAIL: malformed digest reached destructive cache handling"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<'EOF'
[project]
name = "consumer"
version = "0.1.0"

[dependencies]
EOF
bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null
if [ -f "$CONSUMER_DIR/Seen.lock" ]; then
    echo "FAIL: removing registry dependencies left a stale Seen.lock"
    exit 1
fi

echo "PASS: seen pkg can publish to a local registry and consume the package from Seen.toml"
