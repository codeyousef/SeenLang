#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_byte_streams.py"
CONTRACT="$ROOT_DIR/tests/fixtures/byte/bytes-001c-contract.json"
NATIVE="$ROOT_DIR/seen_std/src/io/bytes.seen"
TEST="$ROOT_DIR/seen_std/tests/byte/bytes-001c.seen"
fail(){ echo "FAIL: BYTES-001C contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" || fail "Python syntax"
python3 "$CHECK" "$CONTRACT" >/dev/null || fail "contract fixture"
python3 "$CHECK" "$CONTRACT" \
    --fuzz-seconds "${SEEN_BYTES_001C_FUZZ_SECONDS:-1}" --seed 1101 |
    grep -Fq 'seed=1101' || fail "seeded chunk fuzz"
python3 "$CHECK" "$CONTRACT" --benchmark |
    grep -Fq 'warmups=5 samples=30 hard_gate=5%' || fail "5/30/5 benchmark"

for symbol in 'trait Reader' 'trait Writer' 'trait Seeker' 'trait BufRead' \
    'trait Flush' ByteReader ByteWriter tryByteWriterWithLimit \
    validateByteOperation BYTE_SEEK_START BYTE_SEEK_CURRENT BYTE_SEEK_END; do
    grep -Fq "$symbol" "$NATIVE" || fail "native symbol $symbol"
done
for name in BYTES-001C_happy BYTES-001C_partial BYTES-001C_invalid \
    BYTES-001C_limit BYTES-001C_cancel BYTES-001C_cleanup; do
    grep -Fq "$name" "$TEST" || fail "native case $name"
done
grep -Fq 'seen_std/tests/byte/bytes-001c.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 test wiring"
grep -Fq 'seen_std/examples/byte_streams.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 example wiring"
grep -Fq seen-byte-streams-v1 \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
grep -Fq 'Synchronous byte I/O' \
    "$ROOT_DIR/docs/api-reference/stdlib-modules.md" || fail "API documentation"
grep -Fq 'synchronous byte stream traits' "$ROOT_DIR/CHANGELOG.md" ||
    fail "changelog"
[ -z "$(jobs -pr)" ] || fail "BYTES-001C_cleanup"
echo "PASS: BYTES-001C synchronous stream traits"
