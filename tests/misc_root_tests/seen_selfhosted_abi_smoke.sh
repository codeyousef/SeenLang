#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_selfhosted_abi_smoke.XXXXXX)"
VMEM_KB="${SEEN_SELFHOSTED_ABI_VMEM_KB:-${SEEN_MAIN_VMEM_KB:-2097152}}"
trap 'rm -rf "$TMP_DIR"' EXIT

COMPILER="${SEEN_SELFHOSTED_ABI_COMPILER:-${COMPILER:-}}"
if [ -z "$COMPILER" ]; then
    if [ -x "$ROOT_DIR/compiler_seen/target/seen" ]; then
        COMPILER="$ROOT_DIR/compiler_seen/target/seen"
    else
        COMPILER="$(command -v seen || true)"
    fi
fi

if [ -z "$COMPILER" ] || [ ! -x "$COMPILER" ]; then
    echo "FAIL: no executable Seen compiler for self-hosted ABI smoke" >&2
    exit 1
fi

PROJECT_DIR="$TMP_DIR/selfhost_abi"
OUTPUT_FILE="$PROJECT_DIR/selfhost_abi_smoke"
CHECK_LOG="$PROJECT_DIR/check.log"
COMPILE_LOG="$PROJECT_DIR/compile.log"
RUN_LOG="$PROJECT_DIR/run.log"
mkdir -p "$PROJECT_DIR/selfhost_abi"

cat >"$PROJECT_DIR/Seen.toml" <<'EOF'
[project]
name = "selfhost_abi"
version = "0.1.0"
language = "en"

modules = [
    "selfhost_abi/helpers.seen",
]

[build]
entry = "main.seen"
EOF

cat >"$PROJECT_DIR/selfhost_abi/helpers.seen" <<'EOF'
class AbiSnapshot {
    var names: Array<String>
    var ids: Array<Int>
    var label: String

    static fun new(names: Array<String>, ids: Array<Int>,
        label: String) r: AbiSnapshot {

        return AbiSnapshot { names: names, ids: ids, label: label }
    }

    fun score() r: Int {
        return names.length() + ids.length() + label.length()
    }
}

class OwnerStateBox {
    var funcNames: Array<String>
    var funcRetTypes: Array<String>

    static fun new() r: OwnerStateBox {
        return OwnerStateBox {
            funcNames: Array<String>(),
            funcRetTypes: Array<String>()
        }
    }

    fun add(name: String, retType: String) r: Void {
        this.funcNames.push(name)
        this.funcRetTypes.push(retType)
    }

    fun count() r: Int {
        return funcNames.length() + funcRetTypes.length()
    }
}

fun prepareIdentitySnapshot(names: Array<String>, ids: Array<Int>,
    label: String) r: AbiSnapshot {

    return AbiSnapshot.new(names, ids, label)
}

fun registerIdentity(snapshot: AbiSnapshot, owner: OwnerStateBox,
    extraNames: Array<String>, extraIds: Array<Int>, prefix: String) r: Int {

    owner.add(prefix, snapshot.label)
    return snapshot.score() + owner.count() + extraNames.length() +
        extraIds.length()
}
EOF

cat >"$PROJECT_DIR/main.seen" <<'EOF'
import selfhost_abi.helpers.{OwnerStateBox, prepareIdentitySnapshot, registerIdentity}

fun main() r: Int {
    var names = Array<String>()
    names.push("alpha")
    names.push("beta")

    var ids = Array<Int>()
    ids.push(7)
    ids.push(11)

    var extraNames = Array<String>()
    extraNames.push("gamma")

    var extraIds = Array<Int>()
    extraIds.push(13)

    let owner = OwnerStateBox.new()
    let snapshot = prepareIdentitySnapshot(names, ids, "label")
    let score = registerIdentity(snapshot, owner, extraNames, extraIds, "fn")
    if score == 13 {
        return 0
    }
    return 1
}
EOF

run_capped() {
    (
        ulimit -v "$VMEM_KB" 2>/dev/null || true
        "$@"
    )
}

if ! (
    cd "$PROJECT_DIR" &&
    run_capped timeout 180 "$COMPILER" check main.seen >"$CHECK_LOG" 2>&1
); then
    echo "FAIL: self-hosted ABI check smoke failed; log: $CHECK_LOG" >&2
    tail -n 120 "$CHECK_LOG" >&2 || true
    exit 1
fi

if ! (
    cd "$PROJECT_DIR" &&
    run_capped timeout 180 "$COMPILER" compile main.seen "$OUTPUT_FILE" \
        --fast --no-cache --no-fork >"$COMPILE_LOG" 2>&1
); then
    echo "FAIL: self-hosted ABI compile smoke failed; log: $COMPILE_LOG" >&2
    tail -n 120 "$COMPILE_LOG" >&2 || true
    exit 1
fi

if ! run_capped "$OUTPUT_FILE" >"$RUN_LOG" 2>&1; then
    echo "FAIL: self-hosted ABI run smoke failed; log: $RUN_LOG" >&2
    tail -n 120 "$RUN_LOG" >&2 || true
    exit 1
fi

echo "PASS: self-hosted ABI smoke fixture"
