#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_byte_adapters.py"
CONTRACT="$ROOT_DIR/tests/fixtures/byte/bytes-001d-contract.json"
NATIVE="$ROOT_DIR/seen_std/src/io/bytes.seen"
TEST="$ROOT_DIR/seen_std/tests/byte/bytes-001d.seen"
fail(){ echo "FAIL: BYTES-001D contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" || fail "Python syntax"
python3 "$CHECK" "$CONTRACT" >/dev/null || fail "contract fixture"
python3 "$CHECK" "$CONTRACT" \
    --fuzz-seconds "${SEEN_BYTES_001D_FUZZ_SECONDS:-1}" --seed 1101 |
    grep -Fq 'seed=1101' || fail "seeded adapter fuzz"
python3 "$CHECK" "$CONTRACT" --benchmark |
    grep -Fq 'warmups=5 samples=30 hard_gate=5%' || fail "5/30/5 benchmark"

for symbol in CopyResult LimitedReader CountingReader TeeReader HashingWriter \
    BufferedReader BufferedWriter copyLimited tryTeeReader tryHashingWriter \
    tryBufferedWriter; do
    grep -Fq "$symbol" "$NATIVE" || fail "native symbol $symbol"
done
for name in BYTES-001D_happy BYTES-001D_partial BYTES-001D_invalid \
    BYTES-001D_limit BYTES-001D_cancel BYTES-001D_cleanup; do
    grep -Fq "$name" "$TEST" || fail "native case $name"
done
grep -Fq 'seen_std/tests/byte/bytes-001d.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 test wiring"
grep -Fq 'seen_std/examples/byte_adapters.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 example wiring"
grep -Fq seen-byte-adapters-v1 \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
grep -Fq 'Bounded adapters include' \
    "$ROOT_DIR/docs/api-reference/stdlib-modules.md" || fail "API documentation"
grep -Fq 'bounded limited, counting, tee' "$ROOT_DIR/CHANGELOG.md" ||
    fail "changelog"
[ -z "$(jobs -pr)" ] || fail "BYTES-001D_cleanup"
echo "PASS: BYTES-001D bounded stream adapters"
