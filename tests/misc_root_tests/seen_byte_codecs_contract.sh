#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_byte_codecs.py"
CONTRACT="$ROOT_DIR/tests/fixtures/byte/bytes-001e-contract.json"
NATIVE="$ROOT_DIR/seen_std/src/io/bytes.seen"
TEST="$ROOT_DIR/seen_std/tests/byte/bytes-001e.seen"
fail(){ echo "FAIL: BYTES-001E contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" || fail "Python syntax"
python3 "$CHECK" "$CONTRACT" >/dev/null || fail "contract fixture"
python3 "$CHECK" "$CONTRACT" --fuzz-seconds "${SEEN_BYTES_001E_FUZZ_SECONDS:-1}" --seed 1101 |
    grep -Fq 'seed=1101' || fail "seeded codec fuzz"
python3 "$CHECK" "$CONTRACT" --benchmark |
    grep -Fq 'warmups=5 samples=30 hard_gate=5%' || fail "5/30/5 benchmark"

for symbol in decodeUtf8Strict decodeUtf8Lossy encodeUtf8 readEndianBits \
    writeEndianBits readU16LittleEndian readU32BigEndian readI64LittleEndian \
    readFloat16BitsLittleEndian readBFloat16BitsBigEndian \
    readFloat64BitsLittleEndian writeFloat64BitsBigEndian; do
    grep -Fq "$symbol" "$NATIVE" || fail "native symbol $symbol"
done
for name in BYTES-001E_happy BYTES-001E_partial BYTES-001E_invalid \
    BYTES-001E_limit BYTES-001E_cancel BYTES-001E_cleanup; do
    grep -Fq "$name" "$TEST" || fail "native case $name"
done
grep -Fq 'seen_std/tests/byte/bytes-001e.seen' "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 test wiring"
grep -Fq 'seen_std/examples/byte_codecs.seen' "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 example wiring"
grep -Fq seen-byte-codecs-v1 "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || fail "compatibility binding"
grep -Fq 'UTF-8 codecs provide strict validation' "$ROOT_DIR/docs/api-reference/stdlib-modules.md" || fail "API documentation"
grep -Fq 'strict and lossy bounded UTF-8 codecs' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
[ -z "$(jobs -pr)" ] || fail "BYTES-001E_cleanup"
echo "PASS: BYTES-001E UTF-8 and endian codecs"
