#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_byte_slices.py"
CONTRACT="$ROOT_DIR/tests/fixtures/byte/bytes-001a-contract.json"
NATIVE="$ROOT_DIR/seen_std/src/io/bytes.seen"
TEST="$ROOT_DIR/seen_std/tests/byte/bytes-001a.seen"
fail(){ echo "FAIL: BYTES-001A contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" || fail "Python syntax"
python3 "$CHECK" "$CONTRACT" >/dev/null || fail "contract fixture"
python3 "$CHECK" "$CONTRACT" \
    --fuzz-seconds "${SEEN_BYTES_001A_FUZZ_SECONDS:-1}" --seed 1101 |
    grep -Fq 'seed=1101' || fail "seeded fuzz"
python3 "$CHECK" "$CONTRACT" --benchmark |
    grep -Fq 'warmups=5 samples=30 hard_gate=5%' || fail "5/30/5 benchmark"

for symbol in ByteBuffer ByteSlice MutableByteSlice IoError \
    validateByteRange validateByteRangeInContext BYTE_MAX_COUNT; do
    grep -Fq "$symbol" "$NATIVE" || fail "native symbol $symbol"
done
for code in byte.invalid byte.limit byte.cancelled byte.timeout \
    byte.unavailable byte.unsupported_platform; do
    grep -Fq "$code" "$NATIVE" || fail "stable code $code"
done
for name in BYTES-001A_happy BYTES-001A_partial BYTES-001A_invalid \
    BYTES-001A_limit BYTES-001A_cancel BYTES-001A_cleanup; do
    grep -Fq "$name" "$TEST" || fail "native case $name"
done
grep -Fq 'src/io/bytes.seen' "$ROOT_DIR/seen_std/Seen.toml" ||
    fail "standard-library manifest"
grep -Fq 'seen_std/tests/byte/bytes-001a.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 wiring"
grep -Fq seen-byte-slices-v1 \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
grep -Fq '`io/bytes`' "$ROOT_DIR/docs/api-reference/stdlib-modules.md" ||
    fail "API documentation"
grep -Fq 'native checked byte ownership' "$ROOT_DIR/CHANGELOG.md" ||
    fail "changelog"
[ -z "$(jobs -pr)" ] || fail "BYTES-001A_cleanup"
echo "PASS: BYTES-001A checked byte ownership and slices"
