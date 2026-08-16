#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-pkg-prebuild-string-fields
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-pkg-prebuild-string-fields.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-pkg-prebuild-string-fields.*)
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

bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
    "$TMP_DIR/dep" "$TMP_DIR/dep_artifact" >/dev/null

grep -F "src/spec.seen" "$TMP_DIR/dep_artifact/objects.tsv" >/dev/null
grep -F "module	src/spec.seen" \
    "$TMP_DIR/dep_artifact/interface.index.tsv" >/dev/null

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

bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
    "$TMP_DIR/editor" "$TMP_DIR/editor_artifact" >/dev/null

grep -F "function	src/editor.seen	public	fieldFilterText" "$TMP_DIR/editor_artifact/interface.index.tsv" >/dev/null
grep -F "function	src/editor.seen	public	packageBoundaryStringProbe" "$TMP_DIR/editor_artifact/interface.index.tsv" >/dev/null

echo "PASS: prebuilt package artifacts preserve dependency String field access"
