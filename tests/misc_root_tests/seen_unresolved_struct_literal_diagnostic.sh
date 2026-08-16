#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-unresolved-struct-literal
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-unresolved-struct-literal.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-unresolved-struct-literal.*)
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

cat >"$TMP_DIR/missing_struct_arg.seen" <<'EOF'
fun acceptVertex(value: Int) {
}

fun main() {
    acceptVertex(MeshVertex {
        posX: 1.0,
        posY: 2.0
    })
}
EOF

LOG="$TMP_DIR/compile.log"
if bash "$ATTESTED_SEEN" "$COMPILER" compile \
    "$TMP_DIR/missing_struct_arg.seen" "$TMP_DIR/out" \
    --no-cache >"$LOG" 2>&1; then
    echo "FAIL: unresolved struct literal compiled successfully"
    cat "$LOG"
    exit 1
fi

if ! grep -F 'unresolved type `MeshVertex`' "$LOG" >/dev/null; then
    echo "FAIL: unresolved struct literal did not report a Seen diagnostic"
    cat "$LOG"
    exit 1
fi

if grep -F "inferred layout for unknown struct type" "$LOG" >/dev/null ||
    grep -F "/usr/bin/opt:" "$LOG" >/dev/null ||
    grep -F "/usr/bin/llc:" "$LOG" >/dev/null; then

    echo "FAIL: unresolved struct literal reached invalid LLVM IR"
    cat "$LOG"
    exit 1
fi

echo "PASS: unresolved struct literals fail before LLVM IR emission"
