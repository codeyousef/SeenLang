#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_byte_storage.py"
CONTRACT="$ROOT_DIR/tests/fixtures/byte/bytes-001b-contract.json"
NATIVE="$ROOT_DIR/seen_std/src/io/bytes.seen"
TEST="$ROOT_DIR/seen_std/tests/byte/bytes-001b.seen"
fail(){ echo "FAIL: BYTES-001B contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" || fail "Python syntax"
python3 "$CHECK" "$CONTRACT" >/dev/null || fail "contract fixture"
python3 "$CHECK" "$CONTRACT" \
    --fuzz-seconds "${SEEN_BYTES_001B_FUZZ_SECONDS:-1}" --seed 1101 |
    grep -Fq 'seed=1101' || fail "seeded growth fuzz"
python3 "$CHECK" "$CONTRACT" --benchmark |
    grep -Fq 'warmups=5 samples=30 hard_gate=5%' || fail "5/30/5 benchmark"

for symbol in BYTE_MAX_CAPACITY BYTE_DEFAULT_ALIGNMENT BYTE_MAX_ALIGNMENT \
    isValidByteAlignment validateByteCapacity tryByteBufferWithCapacity \
    tryZeroedByteBuffer capacity alignment reserve push append toArray; do
    grep -Fq "$symbol" "$NATIVE" || fail "native symbol $symbol"
done
for name in BYTES-001B_happy BYTES-001B_partial BYTES-001B_invalid \
    BYTES-001B_limit BYTES-001B_cancel BYTES-001B_cleanup; do
    grep -Fq "$name" "$TEST" || fail "native case $name"
done
grep -Fq 'seen_std/tests/byte/bytes-001b.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 wiring"
grep -Fq seen-byte-storage-v1 \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
grep -Fq 'tryByteBufferWithCapacity' \
    "$ROOT_DIR/docs/api-reference/stdlib-modules.md" || fail "API documentation"
grep -Fq 'native bounded growable byte storage' "$ROOT_DIR/CHANGELOG.md" ||
    fail "changelog"
[ -z "$(jobs -pr)" ] || fail "BYTES-001B_cleanup"
echo "PASS: BYTES-001B native bounded buffer storage"
