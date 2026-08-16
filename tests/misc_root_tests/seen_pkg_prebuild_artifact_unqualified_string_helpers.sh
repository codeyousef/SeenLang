#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-pkg-prebuild-unqualified-strings
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-pkg-prebuild-unqualified-strings.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-pkg-prebuild-unqualified-strings.*)
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

bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
    "$TMP_DIR/facade" "$TMP_DIR/build/facade" >/dev/null

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

bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
    "$TMP_DIR/consumer" "$TMP_DIR/build/consumer" >/dev/null

grep -F "function	src/use_dependency.seen	package	reproConsumerJoin" \
    "$TMP_DIR/build/consumer/interface.index.tsv" >/dev/null

echo "PASS: prebuilt package artifacts preserve unqualified dependency String helper ABI"
