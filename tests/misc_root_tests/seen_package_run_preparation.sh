#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}"
TMP_DIR="$(mktemp -d /tmp/seen_package_run_prepare.XXXXXX)"

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if [[ ! -x "$COMPILER" ]]; then
    echo "FAIL: compiler is missing or not executable: $COMPILER" >&2
    exit 1
fi

PROJECT="$TMP_DIR/project ; package args stay data"
HELPER="$TMP_DIR/helper ; never a shell command"
CAPTURE="$TMP_DIR/captured.json"
mkdir -p "$PROJECT/src"

cat > "$PROJECT/Seen.toml" <<'TOML'
manifest-version = 1

[project]
name = "run_prepare_probe"
version = "0.1.0"

[dependencies]
TOML

# This source must never reach parsing because the deliberately failing helper
# is required to run before the default JIT module-collection path.
cat > "$PROJECT/src/main.seen" <<'SEEN'
this is deliberately not valid Seen source
SEEN

cat > "$HELPER" <<'PY'
#!/usr/bin/env python3
import json
import os
import pathlib
import sys

if len(sys.argv) != 3 or sys.argv[1] != "--request":
    raise SystemExit(97)
request_path = pathlib.Path(sys.argv[2])
payload = request_path.read_bytes()
cursor = 0


def line():
    global cursor
    end = payload.index(b"\n", cursor)
    value = payload[cursor:end]
    cursor = end + 1
    return value


if line() != b"SEENPKG1":
    raise SystemExit(96)
count = int(line())
arguments = []
for _ in range(count):
    length = int(line())
    value = payload[cursor:cursor + length]
    cursor += length
    if payload[cursor:cursor + 1] != b"\n":
        raise SystemExit(95)
    cursor += 1
    arguments.append(value.decode("utf-8"))
if cursor != len(payload):
    raise SystemExit(94)
pathlib.Path(os.environ["SEEN_RUN_PREP_CAPTURE"]).write_text(
    json.dumps({"arguments": arguments, "request_path": str(request_path)}),
    encoding="utf-8",
)
raise SystemExit(73)
PY
chmod 755 "$HELPER"

set +e
SEEN_PACKAGE_CLIENT="$HELPER" SEEN_RUN_PREP_CAPTURE="$CAPTURE" \
    "$COMPILER" run "$PROJECT/src/main.seen" --frozen \
    >"$TMP_DIR/compiler.out" 2>&1
status=$?
set -e

if [[ "$status" -eq 0 ]]; then
    cat "$TMP_DIR/compiler.out" >&2
    echo "FAIL: default run unexpectedly succeeded after helper failure" >&2
    exit 1
fi
if [[ ! -f "$CAPTURE" ]]; then
    cat "$TMP_DIR/compiler.out" >&2
    echo "FAIL: default run did not invoke the package helper before parsing" >&2
    exit 1
fi

python3 - "$CAPTURE" "$PROJECT/Seen.toml" <<'PY'
import json
import pathlib
import sys

capture = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
expected = [
    "--expect-version",
    "0.10.0",
    "fetch",
    sys.argv[2],
    "--quiet",
    "--frozen",
]
if capture["arguments"] != expected:
    raise SystemExit(
        f"default run package request mismatch:\nexpected={expected!r}\n"
        f"actual={capture['arguments']!r}"
    )
if pathlib.Path(capture["request_path"]).exists():
    raise SystemExit("runtime left the package request file behind")
PY

echo "PASS: default run prepares packages in frozen mode before module collection"
