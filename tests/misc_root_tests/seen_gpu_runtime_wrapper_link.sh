#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="/tmp/seen_gpu_runtime_wrapper_link"
CAP_KB="$(awk '/MemAvailable/ { v=int($2/2); if (v>8388608) v=8388608; print v }' /proc/meminfo)"
SEEN_BIN="${SEEN_BIN:-seen}"

rm -rf "$TMP_DIR"
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
    ulimit -v "$CAP_KB"
    "$SEEN_BIN" compile "$TMP_DIR/src/main.seen" "$TMP_DIR/target/repro" --no-fork
)

"$TMP_DIR/target/repro" >/dev/null
echo "PASS: GPU wrapper imports link seen_gpu runtime"
