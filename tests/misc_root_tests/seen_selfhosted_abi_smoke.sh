#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
export SEEN_COMPILER_SOURCE_ROOT
COMPILER="${SEEN_SELFHOSTED_ABI_COMPILER:-${COMPILER:-}}"
if [ -z "$COMPILER" ]; then
    if [ -x "$ROOT_DIR/compiler_seen/target/seen" ]; then
        COMPILER="$ROOT_DIR/compiler_seen/target/seen"
    fi
fi
if [ -z "$COMPILER" ] || [ ! -x "$COMPILER" ]; then
    echo "FAIL: no executable Seen compiler for self-hosted ABI smoke" >&2
    exit 1
fi
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-selfhosted-abi-smoke
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
VMEM_KB="${SEEN_SELFHOSTED_ABI_VMEM_KB:-$SEEN_MAIN_VMEM_KB}"
case "$VMEM_KB" in
    ''|*[!0-9]*)
        echo "RESOURCE STOP: invalid self-hosted ABI memory cap" >&2
        exit 126
        ;;
esac
if [ "$VMEM_KB" -le 0 ] || [ "$VMEM_KB" -gt "$SEEN_MAIN_VMEM_KB" ]; then
    echo "RESOURCE STOP: self-hosted ABI cap must not exceed the verified main cap" >&2
    exit 126
fi
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-selfhosted-abi-smoke.XXXXXX")"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-selfhosted-abi-smoke.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) return 1 ;;
    esac
}
trap cleanup EXIT

PROJECT_DIR="$TMP_DIR/selfhost_abi"
OUTPUT_FILE="$PROJECT_DIR/selfhost_abi_smoke"
CHECK_LOG="$PROJECT_DIR/check.log"
COMPILE_LOG="$PROJECT_DIR/compile.log"
RUN_LOG="$PROJECT_DIR/run.log"
mkdir -p "$PROJECT_DIR/selfhost_abi"
case "$PROJECT_DIR" in
    "$ROOT_DIR"/*) ;;
    *) echo "RESOURCE STOP: self-hosted ABI project escaped the repository root" >&2; exit 126 ;;
esac
PROJECT_ENTRY="${PROJECT_DIR#"$ROOT_DIR"/}/main.seen"

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
        if ! ulimit -S -v "$VMEM_KB" 2>/dev/null; then
            echo "RESOURCE STOP: could not apply self-hosted ABI virtual-memory cap (${VMEM_KB} KiB)" >&2
            exit 126
        fi
        active_vmem=$(ulimit -S -v 2>/dev/null || true)
        case "$active_vmem" in
            ''|*[!0-9]*)
                echo "RESOURCE STOP: could not read back self-hosted ABI virtual-memory cap" >&2
                exit 126
                ;;
        esac
        if [ "$active_vmem" -gt "$VMEM_KB" ]; then
            echo "RESOURCE STOP: self-hosted ABI virtual-memory cap read-back exceeds ${VMEM_KB} KiB" >&2
            exit 126
        fi
        "$@"
    )
}

normalized_failure_status() {
    case "$1" in
        124|125|126|137|143) printf '%s\n' "$1" ;;
        *) printf '1\n' ;;
    esac
}

check_status=0
(
    cd "$ROOT_DIR" &&
    run_capped timeout 180 bash "$ATTESTED_SEEN" "$COMPILER" \
        check "$PROJECT_ENTRY" >"$CHECK_LOG" 2>&1
) || check_status=$?
if [ "$check_status" -ne 0 ]; then
    echo "FAIL: self-hosted ABI check smoke failed; log: $CHECK_LOG" >&2
    tail -n 120 "$CHECK_LOG" >&2 || true
    exit "$(normalized_failure_status "$check_status")"
fi

compile_status=0
(
    cd "$ROOT_DIR" &&
    run_capped timeout 180 bash "$ATTESTED_SEEN" "$COMPILER" \
        compile "$PROJECT_ENTRY" "$OUTPUT_FILE" --fast --no-cache \
        >"$COMPILE_LOG" 2>&1
) || compile_status=$?
if [ "$compile_status" -ne 0 ]; then
    echo "FAIL: self-hosted ABI compile smoke failed; log: $COMPILE_LOG" >&2
    tail -n 120 "$COMPILE_LOG" >&2 || true
    exit "$(normalized_failure_status "$compile_status")"
fi

run_status=0
run_capped "$OUTPUT_FILE" >"$RUN_LOG" 2>&1 || run_status=$?
if [ "$run_status" -ne 0 ]; then
    echo "FAIL: self-hosted ABI run smoke failed; log: $RUN_LOG" >&2
    tail -n 120 "$RUN_LOG" >&2 || true
    exit "$(normalized_failure_status "$run_status")"
fi

echo "PASS: self-hosted ABI smoke fixture"
