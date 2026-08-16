#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-pkg-prebuild-concurrent-temp-paths
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-pkg-prebuild-concurrent.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-pkg-prebuild-concurrent.*)
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

if ! command -v opt >/dev/null 2>&1 || ! command -v llc >/dev/null 2>&1; then
    echo "SKIP: LLVM opt/llc not available"
    exit 0
fi

if command -v llvm-nm >/dev/null 2>&1; then
    NM_TOOL=llvm-nm
else
    NM_TOOL=nm
fi

make_pkg() {
    local name="$1"
    local base="$2"
    local pkg="$TMP_DIR/$name"

    mkdir -p "$pkg/src"
    cat >"$pkg/Seen.toml" <<EOF
[project]
name = "$name"
version = "0.1.0"
language = "en"
modules = [
    "src/value0.seen",
    "src/value1.seen",
    "src/value2.seen",
    "src/value3.seen"
]
EOF

    local idx=0
    while [ "$idx" -lt 4 ]; do
        cat >"$pkg/src/value$idx.seen" <<EOF
pub fun ${name}_value_${idx}() r: Int {
    return $((base + idx))
}
EOF
        idx=$((idx + 1))
    done
}

check_artifact() {
    local name="$1"
    local artifact="$TMP_DIR/${name}_artifact"
    local manifest="$artifact/objects.tsv"

    if [ ! -f "$manifest" ]; then
        echo "FAIL: missing object manifest for $name"
        cat "$TMP_DIR/${name}.log" || true
        exit 1
    fi

    local idx=0
    while [ "$idx" -lt 4 ]; do
        local rel_obj
        rel_obj="$(awk -F '	' -v src="src/value${idx}.seen" '$2 == src {print $1}' "$manifest")"
        if [ -z "$rel_obj" ] || [ ! -f "$artifact/$rel_obj" ]; then
            echo "FAIL: missing object for $name value$idx"
            cat "$manifest"
            exit 1
        fi
        if ! "$NM_TOOL" --defined-only "$artifact/$rel_obj" | grep -F "${name}_value_${idx}" >/dev/null; then
            echo "FAIL: object for $name value$idx was clobbered by another concurrent prebuild"
            "$NM_TOOL" --defined-only "$artifact/$rel_obj" || true
            exit 1
        fi
        idx=$((idx + 1))
    done
}

make_pkg alpha 100
make_pkg beta 200

(
    bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
        "$TMP_DIR/alpha" "$TMP_DIR/alpha_artifact" \
        >"$TMP_DIR/alpha.log" 2>&1
) &
alpha_pid=$!

(
    bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
        "$TMP_DIR/beta" "$TMP_DIR/beta_artifact" \
        >"$TMP_DIR/beta.log" 2>&1
) &
beta_pid=$!

alpha_status=0
beta_status=0
wait "$alpha_pid" || alpha_status=$?
wait "$beta_pid" || beta_status=$?

if [ "$alpha_status" -ne 0 ] || [ "$beta_status" -ne 0 ]; then
    echo "FAIL: concurrent package prebuild failed"
    cat "$TMP_DIR/alpha.log" || true
    cat "$TMP_DIR/beta.log" || true
    exit 1
fi

check_artifact alpha
check_artifact beta

echo "PASS: concurrent package prebuilds keep per-invocation temp artifacts isolated"
