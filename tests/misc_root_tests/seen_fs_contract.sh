#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ ! -x "$COMPILER" ]; then
    COMPILER="$ROOT_DIR/bootstrap/stage1_frozen"
fi
if [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != 1 ]; then
    exec "$ROOT_DIR/scripts/run_with_project_artifacts.sh" seen-fs-contract -- \
        bash "$0" "$@"
fi
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ]; then
    exec "$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" \
        --label "FS-001A-H and SEEN-IO-001B contract" --timeout-secs 900 -- \
        bash "$0" "$@"
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

NO_COLOR=1 SEEN_COLOR=never SEEN_NO_FORK=1 SEEN_DATA_PATH="$ROOT_DIR/languages" \
    "$COMPILER" check "$ROOT_DIR/seen_std/tests/fs/fs_001_contract.seen"
BUILD_DIR="${SEEN_ARTIFACT_ROOT:?}/filesystem-seen-contract"
mkdir -p "$BUILD_DIR"
timeout --foreground --kill-after=10s 600s \
    env NO_COLOR=1 SEEN_COLOR=never SEEN_NO_FORK=1 \
    SEEN_DATA_PATH="$ROOT_DIR/languages" \
    "$COMPILER" compile "$ROOT_DIR/seen_std/tests/fs/fs_001_contract.seen" \
    "$BUILD_DIR/test" --release --lto thin --no-cache --no-fork \
    --jobs 1 --opt-jobs 1
timeout --foreground --kill-after=5s 60s "$BUILD_DIR/test"
bash "$ROOT_DIR/tests/misc_root_tests/seen_fs_runtime_contract.sh"
python3 "$ROOT_DIR/scripts/check_native_boundaries.py" \
    "$ROOT_DIR/docs/architecture/native-boundaries.json"

grep -Fq 'class OsString' "$ROOT_DIR/seen_std/src/fs/types.seen"
grep -Fq '@move' "$ROOT_DIR/seen_std/src/fs/file.seen"
grep -Fq 'fun atomicReplace' "$ROOT_DIR/seen_std/src/fs/operations.seen"
grep -Fq 'fun removeRecursively' "$ROOT_DIR/seen_std/src/fs/operations.seen"
grep -Fq 'fun moveAcrossFilesystems' "$ROOT_DIR/seen_std/src/fs/operations.seen"
grep -Fq 'fun readGather' "$ROOT_DIR/seen_std/src/io/direct.seen"
grep -Fq 'fallbackCount' "$ROOT_DIR/seen_std/src/fs/types.seen"
echo "PASS: FS-001A-H and SEEN-IO-001B contract"
