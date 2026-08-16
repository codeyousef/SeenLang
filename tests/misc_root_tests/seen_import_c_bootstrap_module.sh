#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-import-c-bootstrap-module
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-import-c-bootstrap-module.XXXXXX")"
PROBE_SRC="$TMP_DIR/compiler_seen/src/seen_import_c_bootstrap_probe.seen"
PROBE_BIN="$TMP_DIR/probe_bin"
RUN_OUT="$TMP_DIR/run.out"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-import-c-bootstrap-module.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR" ;;
        *) return 1 ;;
    esac
}

trap cleanup EXIT
mkdir -p "$(dirname -- "$PROBE_SRC")"

cat >"$PROBE_SRC" <<'EOF'
import tools.c_import_gen.{CImportGenerator, CImportResult}

fun main() r: Int {
    let gen = CImportGenerator.new("/tmp/does_not_matter.h")
    let result: CImportResult = CImportResult.new()

    if not result.isSuccessful() {
        println("FAIL: new CImportResult should start successful")
        return 1
    }

    if gen.headerPath != "/tmp/does_not_matter.h" {
        println("FAIL: CImportGenerator.new lost headerPath")
        return 1
    }

    println("PASS: import-c bootstrap module compiles")
    return 0
}
EOF

bash "$ATTESTED_SEEN" "$COMPILER" compile \
    "$PROBE_SRC" "$PROBE_BIN" --fast --no-cache
"$PROBE_BIN" >"$RUN_OUT"

grep -q '^PASS: import-c bootstrap module compiles$' "$RUN_OUT" || {
    echo "FAIL: probe binary did not report success"
    cat "$RUN_OUT"
    exit 1
}

echo "PASS: import-c bootstrap module compiles"
