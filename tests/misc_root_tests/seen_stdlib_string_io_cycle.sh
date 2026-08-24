#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-stdlib-string-io-cycle
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-stdlib-string-io.XXXXXX")"

cleanup() {
    local status=$?
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-stdlib-string-io.*)
            if [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ]; then
                # Local package views are intentionally hardened read-only.
                # Restore only owner directory permissions inside this
                # disposable, identity-checked root before removing it.
                if ! find -P "$TMP_DIR" -type d \
                    -exec chmod u+rwx -- {} +; then

                    echo "ERROR: could not make stdlib regression directories removable: $TMP_DIR" >&2
                    status=1
                elif ! rm -rf -- "$TMP_DIR"; then
                    echo "ERROR: could not clean stdlib regression path: $TMP_DIR" >&2
                    status=1
                fi
            else
                echo "ERROR: refusing to clean unsafe stdlib regression path: $TMP_DIR" >&2
                status=1
            fi
            ;;
        *) status=1 ;;
    esac
    trap - EXIT
    exit "$status"
}
trap cleanup EXIT

run_logged() {
    local label=$1
    local log=$2
    shift 2
    local status=0

    "$@" >"$log" 2>&1 || status=$?
    if [ "$status" -ne 0 ]; then
        echo "FAIL: $label exited with status $status" >&2
        tail -c 32768 "$log" >&2 || true
        return "$status"
    fi
}

if rg -n '^[[:space:]]*import[[:space:]]+str\.string' \
    "$ROOT_DIR/seen_std/src/io/file.seen"; then
    echo "FAIL: io.file reintroduced the reverse str.string dependency" >&2
    exit 1
fi

mkdir -p "$TMP_DIR/src" "$TMP_DIR/target"
cat >"$TMP_DIR/Seen.toml" <<TOML
manifest-version = 1

[project]
name = "stdlib_consumer"
version = "0.1.0"
language = "en"
edition = "2025"
modules = ["src/main.seen"]

[build]
entry = "src/main.seen"
targets = ["native"]

[dependencies]
seen_std = { path = "$ROOT_DIR/seen_std" }
TOML
cat >"$TMP_DIR/src/main.seen" <<'SEEN'
import str.string.{StringBuilder}
import io.file.{writeText, readText, deleteFile}
import json.parser.{parseJson, destroyJsonParseResult}

fun main() r: Int {
    let builder = StringBuilder.new()
    builder.append("seen")
    builder.append("-io")
    let path = "target/seen_stdlib_string_io_cycle.txt"
    let wroteText = writeText(path, builder.toString())
    builder.parts.free()
    builder.free()
    if not wroteText { return 1 }
    if readText(path) != "seen-io" { return 2 }
    let parsed = parseJson("{\"value\":7}")
    if parsed.success != 1 {
        destroyJsonParseResult(parsed, false)
        return 3
    }
    let root = parsed.value.unwrap()
    let value = root.get("value")
    if value.isNone() {
        value.free()
        destroyJsonParseResult(parsed, true)
        return 4
    }
    let number = value.unwrap()
    value.free()
    let numberMatches = number.getInt() == 7
    destroyJsonParseResult(parsed, true)
    if not numberMatches { return 4 }
    if not deleteFile(path) { return 5 }
    return 0
}
SEEN

(
    cd "$TMP_DIR"
    run_logged "external seen_std semantic check" "$TMP_DIR/check.log" \
        timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" check src/main.seen
    run_logged "external seen_std compile" "$TMP_DIR/compile.log" \
        timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile src/main.seen \
        target/string-io --fast --no-cache
)
if rg -n 'cyclic import|error\[E094\]' \
    "$TMP_DIR/check.log" "$TMP_DIR/compile.log"; then
    echo "FAIL: external seen_std consumer reported an import cycle" >&2
    exit 1
fi
(
    cd "$TMP_DIR"
    run_logged "external seen_std executable" "$TMP_DIR/run.log" \
        timeout --foreground --kill-after=5s 30s ./target/string-io
)

echo "PASS: str.string and io.file resolve without a dependency cycle"
