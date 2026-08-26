#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: Qwen prerequisite gate requires the verified serial hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

python3 "$ROOT_DIR/scripts/check_qwen_contracts.py" \
    --schema-dir "$ROOT_DIR/schemas/qwen" \
    --document "$ROOT_DIR/schemas/qwen/qwen-model-lock.schema.json" \
    "$ROOT_DIR/projects/seen_ml/qwen38/locks/qwen-model-lock.json"
python3 "$ROOT_DIR/scripts/check_qwen_contracts.py" \
    --document "$ROOT_DIR/schemas/qwen/qwen-source-lock.schema.json" \
    "$ROOT_DIR/projects/seen_ml/qwen38/locks/qwen-source-lock.json"
if python3 "$ROOT_DIR/scripts/check_qwen_contracts.py" \
    --document "$ROOT_DIR/schemas/qwen/qwen-model-lock.schema.json" \
    "$ROOT_DIR/tests/fixtures/qwen/invalid-model-lock.json" >/dev/null 2>&1; then
    echo "FAIL: hostile model lock was accepted" >&2
    exit 1
fi
python3 "$ROOT_DIR/scripts/check_native_boundaries.py" \
    "$ROOT_DIR/docs/architecture/native-boundaries.json"

"$ROOT_DIR/tests/misc_root_tests/seen_qwen_seen_contracts.sh"

for source in \
    "$ROOT_DIR/seen_std/src/inference/scalars.seen" \
    "$ROOT_DIR/seen_std/src/inference/span.seen" \
    "$ROOT_DIR/seen_std/src/memory/aligned_buffer.seen" \
    "$ROOT_DIR/seen_std/src/memory/mapped_file.seen" \
    "$ROOT_DIR/seen_std/src/crypto/sha256.seen" \
    "$ROOT_DIR/seen_std/src/json/strict.seen" \
    "$ROOT_DIR/seen_std/src/json/canonical.seen" \
    "$ROOT_DIR/seen_std/src/formats/safetensors.seen" \
    "$ROOT_DIR/seen_std/src/accelerator/cuda/mod.seen"; do
    grep -Fq "${source#"$ROOT_DIR/seen_std/"}" "$ROOT_DIR/seen_std/Seen.toml" || {
        echo "FAIL: standard-library manifest omits ${source#"$ROOT_DIR/"}" >&2
        exit 1
    }
done

"$ROOT_DIR/tests/misc_root_tests/seen_qwen_cpu_isolation.sh"
"$ROOT_DIR/tests/misc_root_tests/seen_qwen_native_dependency.sh"
"$ROOT_DIR/tests/misc_root_tests/seen_qwen_mapped_file_large.sh"
if [ "${SEEN_QWEN_REQUIRE_CUDA:-0}" = 1 ]; then
    "$ROOT_DIR/tests/misc_root_tests/seen_qwen_cuda_foundation.sh"
else
    echo "PASS: CPU-only prerequisite gate did not locate or link a CUDA SDK"
fi

echo "PASS: Qwen G0/G1 static, schema, ABI, isolation, and large-file prerequisites"
