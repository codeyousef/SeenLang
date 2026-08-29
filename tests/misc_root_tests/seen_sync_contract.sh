#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
SYNC_ROOT="$ROOT_DIR/seen_std/src/sync"
TEST_ROOT="$ROOT_DIR/seen_std/tests/sync"
LEDGER="$ROOT_DIR/docs/architecture/native-boundaries.json"
SEND_SHARE_SOURCE="$ROOT_DIR/compiler_seen/src/types/send_share.seen"

fail() {
    echo "FAIL: SYNC-001A-H contract: $*" >&2
    exit 1
}

for file in contracts atomic arc lock_order mutex channel mod; do
    [ -f "$SYNC_ROOT/$file.seen" ] && [ ! -L "$SYNC_ROOT/$file.seen" ] ||
        fail "missing native Seen sync module $file"
done

[ -f "$SEND_SHARE_SOURCE" ] && [ ! -L "$SEND_SHARE_SOURCE" ] ||
    fail "missing compiler Send/Share policy module"
send_share_doc_delimiters=$(awk '$0 == "///" { count += 1 } END { print count + 0 }' \
    "$SEND_SHARE_SOURCE")
[ $((send_share_doc_delimiters % 2)) -eq 0 ] ||
    fail "compiler Send/Share policy has an unclosed documentation block"
rg -Fq 'fun sendShareFoundationContract' "$SEND_SHARE_SOURCE" ||
    fail "missing compiler foundation Send/Share implementation"

for symbol in Send Share SyncError SYNC_MAX_CAPACITY \
    SYNC_MAX_SHARED_OWNERS MemoryOrder AtomicInt AtomicBool 'Atomic<T: Share>' \
    'Arc<T: Share>' 'BoundedChannel<T: Send>' 'Mutex<T: Share>' \
    'RwLock<T: Share>' LockOrderTracker; do
    rg -Fq "$symbol" "$SYNC_ROOT" || fail "missing public symbol $symbol"
done

for state_method in loadStrong compareStrong closeStrong valueCopy; do
    rg -Fq "fun $state_method" "$SYNC_ROOT/arc.seen" ||
        fail "ArcState is missing typed $state_method access"
done
if rg -n 'this\.state\.(strong|value)' "$SYNC_ROOT/arc.seen"; then
    fail "Arc bypasses typed ArcState access and risks invalid nested receivers"
fi

for issue in a b c d e f g h; do
    test_file="$TEST_ROOT/sync-001${issue}.seen"
    [ -f "$test_file" ] && [ ! -L "$test_file" ] ||
        fail "missing SYNC-001${issue^^} test"
    for case_name in happy partial invalid limit cancel cleanup; do
        rg -Fq "SYNC-001${issue^^}_${case_name}" "$test_file" ||
            fail "missing SYNC-001${issue^^}_${case_name}"
    done
done

legacy_pattern='(__Mutex(Create|Destroy|Lock|Unlock|TryLock)|__Atomic(Alloc|Load|Store|Add|Sub|CompareExchange)|__Channel(Create|Send|Receive|Close)|seen_(atomic_queue|atomic_stack|rwlock|barrier_(new|wait|destroy)|tls_))'
if rg -n "$legacy_pattern" \
    "$ROOT_DIR/compiler_seen/src" "$ROOT_DIR/seen_std/src" \
    "$ROOT_DIR/seen_runtime/seen_runtime.c" \
    "$ROOT_DIR/seen_runtime/seen_runtime.h"; then
    fail "legacy synchronization production surface remains"
fi

for deleted in atomic_queue atomic_stack barrier mpsc_queue ordering rwlock \
    spsc_queue thread_local; do
    [ ! -e "$SYNC_ROOT/$deleted.seen" ] ||
        fail "legacy sync module remains: $deleted"
done

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$LEDGER" >/dev/null ||
    fail "native-boundary ledger"
rg -Fq 'synchronization-os-adapter' "$LEDGER" ||
    fail "missing native boundary"
rg -Fq 'seen-sync-v1' \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
rg -Fq 'seen-sync-v1' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
rg -Fq 'BoundedChannel<T>' "$ROOT_DIR/docs/api-reference/sync.md" ||
    fail "API documentation"
rg -Fq 'seen_std/examples/synchronization.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" ||
    fail "compiling example is not wired"

[ -z "$(jobs -pr)" ] || fail "cleanup left background jobs"
echo "PASS: SYNC-001A-H native Seen synchronization contract"
