#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_async_byte_streams.py"
CONTRACT="$ROOT_DIR/tests/fixtures/byte/bytes-002a-contract.json"
NATIVE="$ROOT_DIR/seen_std/src/io/bytes.seen"
TEST="$ROOT_DIR/seen_std/tests/byte/bytes-002a.seen"
GENERATOR="$ROOT_DIR/compiler_seen/src/codegen/ir_function_signature_emit.seen"
fail(){ echo "FAIL: BYTES-002A contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" || fail "Python syntax"
python3 "$CHECK" "$CONTRACT" >/dev/null || fail "contract fixture"
python3 "$CHECK" "$CONTRACT" \
    --fuzz-seconds "${SEEN_BYTES_002A_FUZZ_SECONDS:-1}" --seed 1101 |
    grep -Fq 'seed=1101' || fail "seeded async progress fuzz"
python3 "$CHECK" "$CONTRACT" --benchmark |
    grep -Fq 'warmups=5 samples=30 hard_gate=5%' || fail "5/30/5 benchmark"

for symbol in 'trait AsyncReader' 'trait AsyncWriter' 'trait AsyncSeeker' \
    'trait AsyncBufRead' 'trait AsyncFlush' AsyncByteReader AsyncByteWriter \
    asyncRead asyncWrite asyncSeek asyncFillBuf asyncConsume asyncFlush; do
    grep -Fq "$symbol" "$NATIVE" || fail "native symbol $symbol"
done
for name in BYTES-002A_happy BYTES-002A_partial BYTES-002A_invalid \
    BYTES-002A_limit BYTES-002A_cancel BYTES-002A_cleanup; do
    grep -Fq "$name" "$TEST" || fail "native case $name"
done
python3 - "$GENERATOR" <<'PY' || fail "coroutine register order"
import pathlib, sys
text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
alloc = text.index("let coroSize64Reg = getNextRegFn(regBox)")
use = text.index('output.append("  " + coroSize64Reg + " = zext')
if alloc > use:
    raise SystemExit("coro size extension register allocated after use")
PY
grep -Fq 'seen_std/tests/byte/bytes-002a.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 test wiring"
grep -Fq 'seen_std/examples/async_byte_streams.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage-1 example wiring"
grep -Fq seen-async-byte-streams-v1 \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" || fail "compatibility binding"
grep -Fq 'Asynchronous byte I/O' \
    "$ROOT_DIR/docs/api-reference/stdlib-modules.md" || fail "API documentation"
grep -Fq 'asynchronous byte stream traits' "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
[ -z "$(jobs -pr)" ] || fail "BYTES-002A_cleanup"
echo "PASS: BYTES-002A asynchronous stream traits"
