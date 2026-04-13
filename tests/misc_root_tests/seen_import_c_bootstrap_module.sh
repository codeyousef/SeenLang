#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_DIR="$(mktemp -d /tmp/seen_import_c_bootstrap_module.XXXXXX)"
PROBE_SRC="$ROOT_DIR/compiler_seen/src/seen_import_c_bootstrap_probe_tmp.seen"
PROBE_BIN="$TMP_DIR/probe_bin"
RUN_OUT="$TMP_DIR/run.out"

cleanup() {
    rm -rf "$TMP_DIR"
    rm -f "$PROBE_SRC"
}

trap cleanup EXIT

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

"$COMPILER" compile "$PROBE_SRC" "$PROBE_BIN" --fast --no-cache
"$PROBE_BIN" >"$RUN_OUT"

grep -q '^PASS: import-c bootstrap module compiles$' "$RUN_OUT" || {
    echo "FAIL: probe binary did not report success"
    cat "$RUN_OUT"
    exit 1
}

echo "PASS: import-c bootstrap module compiles"
