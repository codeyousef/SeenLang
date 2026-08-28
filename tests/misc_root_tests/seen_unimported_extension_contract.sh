#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
RELEASE_TARGET_CPU=${SEEN_RELEASE_CPU_BASELINE:-x86-64}
case "$RELEASE_TARGET_CPU" in
    x86-64|x86-64-v3) ;;
    *)
        echo "FAIL: unsupported extension contract CPU baseline: $RELEASE_TARGET_CPU" >&2
        exit 2
        ;;
esac
RUNNER="$ROOT_DIR/scripts/run_with_project_artifacts.sh"
BASELINE_CHECKER="$ROOT_DIR/scripts/check_x86_executable_baseline.sh"
INVALID="$ROOT_DIR/tests/fixtures/fel-1535/unimported_extension.seen"
VALID="$ROOT_DIR/tests/fixtures/fel-1535/imported_extension.seen"
mkdir -p "$ROOT_DIR/.seen/agent-tools/fel-1535"
WORK_DIR=$(mktemp -d "$ROOT_DIR/.seen/agent-tools/fel-1535/run.XXXXXX")
cleanup() {
    status=$?
    if [ "$status" -eq 0 ]; then rm -rf "$WORK_DIR"
    else echo "Preserved failed FEL-1535 artifacts: $WORK_DIR" >&2; fi
}
trap cleanup EXIT

run_seen() {
    "$RUNNER" fel-1535 --keep-on-failure -- "$SEEN_BIN" "$@"
}

if run_seen check "$INVALID" >"$WORK_DIR/check-invalid.log" 2>&1; then
    echo "FAIL: unimported extension-style call passed check" >&2
    exit 1
fi
grep -Fq 'E009' "$WORK_DIR/check-invalid.log"
grep -Fq 'unresolved function `lines`' "$WORK_DIR/check-invalid.log"

if run_seen compile "$INVALID" "$WORK_DIR/invalid" --no-cache --no-fork \
    --target-cpu "$RELEASE_TARGET_CPU" --jobs 1 --opt-jobs 1 \
    >"$WORK_DIR/compile-invalid.log" 2>&1; then
    echo "FAIL: unimported extension-style call compiled" >&2
    exit 1
fi
grep -Fq 'unresolved function `lines`' "$WORK_DIR/compile-invalid.log"
test ! -e "$WORK_DIR/invalid"

run_seen check "$VALID" >"$WORK_DIR/check-valid.log" 2>&1
run_seen compile "$VALID" "$WORK_DIR/valid-fast" --fast --no-cache --no-fork \
    --target-cpu "$RELEASE_TARGET_CPU" --jobs 1 --opt-jobs 1 \
    >"$WORK_DIR/compile-fast.log" 2>&1
bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/valid-fast"
"$WORK_DIR/valid-fast"
run_seen compile "$VALID" "$WORK_DIR/valid-release" --release --lto thin \
    --no-fork --no-cache --target-cpu "$RELEASE_TARGET_CPU" \
    --jobs 1 --opt-jobs 1 \
    >"$WORK_DIR/compile-release.log" 2>&1
bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/valid-release"
"$WORK_DIR/valid-release"

if rg -q '(^|[[:space:]])(int3|llvm\.trap)([[:space:](]|$)' "$WORK_DIR"; then
    echo "FAIL: extension-call contract emitted a trap" >&2
    exit 1
fi

echo "PASS: FEL-1535 extension-style import contract"
