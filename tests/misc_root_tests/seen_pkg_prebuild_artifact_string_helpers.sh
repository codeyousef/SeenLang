#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-pkg-prebuild-string-helpers
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-pkg-prebuild-string-helpers.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-pkg-prebuild-string-helpers.*)
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

bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
    "$TMP_DIR/facade" "$TMP_DIR/facade_artifact" >/dev/null

grep -F "src/inspector_model.seen" "$TMP_DIR/facade_artifact/objects.tsv" >/dev/null
grep -F "module	src/inspector_model.seen" \
    "$TMP_DIR/facade_artifact/interface.index.tsv" >/dev/null

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

bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
    "$TMP_DIR/consumer" "$TMP_DIR/consumer_artifact" >/dev/null

grep -F "function	src/consumer.seen	public	packageBoundaryStringHelperProbe" "$TMP_DIR/consumer_artifact/interface.index.tsv" >/dev/null

echo "PASS: prebuilt package artifacts preserve dependency String helper return types"
