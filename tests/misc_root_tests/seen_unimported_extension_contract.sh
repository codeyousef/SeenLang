#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SEEN_BIN=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-unimported-extension-contract
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$SEEN_BIN" -- bash "$0" "$@"
fi
SEEN_BIN="${SEEN_CAPPED_REGRESSION_COMPILER:-$SEEN_BIN}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$SEEN_BIN"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
RELEASE_TARGET_CPU=${SEEN_RELEASE_CPU_BASELINE:-x86-64}
case "$RELEASE_TARGET_CPU" in
    x86-64|x86-64-v3) ;;
    *)
        echo "FAIL: unsupported extension contract CPU baseline: $RELEASE_TARGET_CPU" >&2
        exit 2
        ;;
esac
BASELINE_CHECKER="$ROOT_DIR/scripts/check_x86_executable_baseline.sh"
INVALID="$ROOT_DIR/tests/fixtures/fel-1535/unimported_extension.seen"
VALID="$ROOT_DIR/tests/fixtures/fel-1535/imported_extension.seen"
WORK_DIR=$(mktemp -d "$SEEN_ARTIFACT_ROOT/unimported-extension-contract.XXXXXX")
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ]; then
        case "$WORK_DIR" in
            "$SEEN_ARTIFACT_ROOT"/unimported-extension-contract.*)
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    [ "$(dirname -- "$WORK_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
                rm -rf -- "$WORK_DIR" ;;
            *) return 1 ;;
        esac
    else echo "Preserved failed FEL-1535 artifacts: $WORK_DIR" >&2; fi
}
trap cleanup EXIT

run_seen() {
    bash "$ATTESTED_SEEN" "$SEEN_BIN" "$@"
}

if run_seen check "$INVALID" >"$WORK_DIR/check-invalid.log" 2>&1; then
    echo "FAIL: unimported extension-style call passed check" >&2
    exit 1
fi
grep -Fq 'E009' "$WORK_DIR/check-invalid.log"
grep -Fq 'unresolved function `lines`' "$WORK_DIR/check-invalid.log"

if run_seen compile "$INVALID" "$WORK_DIR/invalid" --no-cache --no-fork \
    --target-cpu "$RELEASE_TARGET_CPU" \
    >"$WORK_DIR/compile-invalid.log" 2>&1; then
    echo "FAIL: unimported extension-style call compiled" >&2
    exit 1
fi
grep -Fq 'unresolved function `lines`' "$WORK_DIR/compile-invalid.log"
test ! -e "$WORK_DIR/invalid"

run_seen check "$VALID" >"$WORK_DIR/check-valid.log" 2>&1
run_seen compile "$VALID" "$WORK_DIR/valid-fast" --fast --no-cache --no-fork \
    --target-cpu "$RELEASE_TARGET_CPU" \
    >"$WORK_DIR/compile-fast.log" 2>&1
bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/valid-fast"
timeout --kill-after=5s 120s "$WORK_DIR/valid-fast"
run_seen compile "$VALID" "$WORK_DIR/valid-release" --release --lto thin \
    --no-fork --no-cache --target-cpu "$RELEASE_TARGET_CPU" \
    >"$WORK_DIR/compile-release.log" 2>&1
bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/valid-release"
timeout --kill-after=5s 120s "$WORK_DIR/valid-release"

if rg -q '(^|[[:space:]])(int3|llvm\.trap)([[:space:](]|$)' "$WORK_DIR"; then
    echo "FAIL: extension-call contract emitted a trap" >&2
    exit 1
fi

echo "PASS: FEL-1535 extension-style import contract"
