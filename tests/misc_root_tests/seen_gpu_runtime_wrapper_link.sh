#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-wrapper-link
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
SEEN_BIN=$COMPILER
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-gpu-runtime-wrapper-link.XXXXXX")"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-gpu-runtime-wrapper-link.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) return 1 ;;
    esac
}
trap cleanup EXIT

mkdir -p "$TMP_DIR/src/graphics" "$TMP_DIR/target"

cat >"$TMP_DIR/Seen.toml" <<'TOML'
[project]
name = "gpu_wrapper_link"
version = "0.1.0"
language = "en"
edition = "2025"

[build]
entry = "src/main.seen"
targets = ["native"]
TOML

cat >"$TMP_DIR/src/main.seen" <<'SEEN'
import graphics.gpu.{gpuIsAvailable}

fun main() -> Int {
    if gpuIsAvailable() {
        println("gpu available")
    } else {
        println("gpu unavailable")
    }
    return 0
}
SEEN

cat >"$TMP_DIR/src/graphics/gpu.seen" <<'SEEN'
extern fun seen_gpu_init() r: Int
extern fun seen_gpu_shutdown() r: Void
extern fun seen_gpu_is_available() r: Int

fun gpuInit() r: Bool {
    return seen_gpu_init() == 1
}

fun gpuShutdown() r: Void {
    seen_gpu_shutdown()
}

fun gpuIsAvailable() r: Bool {
    return seen_gpu_is_available() == 1
}
SEEN

(
    cd "$ROOT_DIR"
    bash "$ATTESTED_SEEN" "$SEEN_BIN" compile \
        "$TMP_DIR/src/main.seen" "$TMP_DIR/target/repro"
)

"$TMP_DIR/target/repro" >/dev/null
echo "PASS: GPU wrapper imports link seen_gpu runtime"
