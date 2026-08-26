#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: ext4/XFS certification requires the verified serial hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

EXT4_ROOT="${SEEN_FS_CERT_EXT4_ROOT:?set SEEN_FS_CERT_EXT4_ROOT to an ext4 directory}"
XFS_ROOT="${SEEN_FS_CERT_XFS_ROOT:?set SEEN_FS_CERT_XFS_ROOT to an XFS directory}"
BUILD_DIR="${SEEN_ARTIFACT_ROOT:?}/filesystem-certification"
mkdir -p "$BUILD_DIR"
clang -std=c11 -O1 -I "$ROOT_DIR/seen_runtime" \
    "$ROOT_DIR/tests/misc_root_tests/seen_fs_runtime_contract.c" \
    "$ROOT_DIR/seen_runtime/seen_runtime.c" \
    -pthread -ldl -lm -o "$BUILD_DIR/test"
for expected in ext4 xfs; do
    if [ "$expected" = ext4 ]; then candidate=$EXT4_ROOT; else candidate=$XFS_ROOT; fi
    [ -d "$candidate" ] && [ ! -L "$candidate" ] && [ -w "$candidate" ] || {
        echo "FAIL: $expected certification root is not a writable real directory" >&2
        exit 1
    }
    actual=$(findmnt -n -o FSTYPE -T "$candidate")
    [ "$actual" = "$expected" ] || {
        echo "FAIL: expected $expected certification root, found $actual" >&2
        exit 1
    }
    work="$candidate/seen-fs-cert-${USER:-user}"
    mkdir "$work"
    "$BUILD_DIR/test" "$work"
    rmdir "$work"
    echo "PASS: $expected filesystem certification"
done
