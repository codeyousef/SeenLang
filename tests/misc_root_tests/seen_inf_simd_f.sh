#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-inf-simd-f
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/inf-simd-f.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=inf-simd-f
FIXTURE_RUN_SECONDS=900
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
FIXTURE_COMPILE_FLAGS=(--bounds-check --simd-report=full)
for profile in fast release undefined address; do
    compile_case "$profile" floating-array "$ROOT_DIR/tests/inference_simd/floating_array_literals.seen"
    compile_case "$profile" quantized "$ROOT_DIR/tests/inference_simd/quantized_corpus.seen"
done
compile_case fast metadata-fuzz "$ROOT_DIR/tests/inference_simd/quantized_metadata_fuzz.seen"
for width in 4 8 16; do
    rg -Fq "llvm.masked.store.v${width}f32.p0" "$TMP_DIR/quantized-release-ir"/*.ll
done
benchmark_args=()
if [ "${SEEN_REQUIRE_QUANTIZED_SIMD_PERF:-0}" = 1 ]; then benchmark_args+=(--enforce); fi
compile_fixture release benchmark "$ROOT_DIR/tests/inference_simd/quantized_benchmark.seen"
benchmark_status=0
cp -- "$TMP_DIR/benchmark-release.compile.log" "$TMP_DIR/00-active.log"
python3 - "$TMP_DIR" "$TMP_DIR/benchmark-release" "${benchmark_args[@]}" <<'PY' || benchmark_status=$?
import json
import resource
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
with (root / "00-active.log").open("a") as log:
    child = subprocess.Popen(sys.argv[2:], stdout=log, stderr=subprocess.STDOUT)
    try:
        status = child.wait(timeout=900)
    except subprocess.TimeoutExpired:
        child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
        status = 124
if status < 0:
    status = 128 - status
# This capture process starts only the benchmark child; no compiler/helper
# subprocess contributes to RUSAGE_CHILDREN's Linux KiB maximum.
usage = {"exit_status": status, "process_peak_rss_bytes": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * 1024,
         "process_peak_rss_status": "linux-waited-native-benchmark-child"}
(root / "benchmark-usage.json").write_text(json.dumps(usage, indent=2) + "\n")
raise SystemExit(status)
PY
cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/benchmark-release.log"
printf '%s\n' "$benchmark_status" > "$TMP_DIR/benchmark-release.run.status"
case "$benchmark_status" in 0|3) ;; *) exit "$benchmark_status" ;; esac
python3 - "$TMP_DIR" "$COMPILER" "$ROOT_DIR" "${SEEN_REQUIRE_QUANTIZED_SIMD_PERF:-0}" "$benchmark_status" <<'PY'
import hashlib
import json
import platform
import sys
from pathlib import Path

root, compiler, source_root = map(Path, sys.argv[1:4])
enforce, status = sys.argv[4] == "1", int(sys.argv[5])
groups = []
current = None
for line in (root / "00-active.log").read_text().splitlines():
    if line.startswith("BENCH: quantized-decode "):
        protocol = dict(field.split("=", 1) for field in line.split()[2:])
        if protocol != {"warmups": "5", "samples": "30", "paired": "true",
                        "control": "scalar", "clock": "monotonic", "blocks_per_sample": "128"}:
            raise SystemExit("FAIL: quantized sample protocol changed")
        current = {"candidate_ns": [], "scalar_ns": []}
        groups.append(current)
    elif line.startswith("BENCH: "):
        if current is None:
            raise SystemExit("FAIL: quantized metrics preceded their sample protocol")
        key, value = line[7:].split("=", 1)
        if key in ("candidate_ns", "scalar_ns"):
            current[key].append(int(value))
        elif key in ("corpus_format_index", "vector_bits", "median_ratio_ppm", "allocator_peak_bytes") and key not in current:
            current[key] = int(value)
        else:
            raise SystemExit("FAIL: duplicate or unknown quantized metric")
expected = {(kind, width) for kind in range(7) for width in (128, 256, 512)}
observed = set()
for group in groups:
    identity = group["corpus_format_index"], group["vector_bits"]
    if identity not in expected or identity in observed:
        raise SystemExit("FAIL: duplicated or foreign quantized measurement case")
    observed.add(identity)
    candidates, controls = group["candidate_ns"], group["scalar_ns"]
    if len(candidates) != 30 or len(controls) != 30 or any(value <= 0 for value in candidates + controls):
        raise SystemExit("FAIL: quantized case lacks thirty positive raw sample pairs")
    ratios = sorted(candidate * 1000000 // control for candidate, control in zip(candidates, controls))
    median = (ratios[14] + ratios[15]) // 2
    if median != group["median_ratio_ppm"]:
        raise SystemExit("FAIL: quantized median disagrees with raw paired measurements")
    group["ratio_q1_ppm"], group["ratio_q3_ppm"] = ratios[7], ratios[22]
    group["within_ceiling"] = median <= 1050000
if observed != expected:
    raise SystemExit("FAIL: quantized performance matrix is incomplete")
within_ceiling = all(group["within_ceiling"] for group in groups)
if status != (3 if enforce and not within_ceiling else 0):
    raise SystemExit("FAIL: quantized native exit status disagrees with hard performance policy")
group_path = next(line.split("::", 1)[1] for line in Path("/proc/self/cgroup").read_text().splitlines() if line.startswith("0::"))
control = Path("/sys/fs/cgroup") / group_path.lstrip("/")
limits = {name: int((control / name).read_text().strip()) for name in ("memory.max", "memory.swap.max", "pids.max")}
if limits["memory.swap.max"] != 0:
    raise SystemExit("FAIL: quantized report observed enabled cgroup swap")
with compiler.open("rb") as source:
    digest = hashlib.file_digest(source, "sha256").hexdigest()
paths = ("seen_std/src/simd/quantized_decode.seen", "seen_std/src/inference/quantized_formats.seen",
         "tests/inference_simd/quantized_benchmark.seen", "tests/inference_simd/quantized_fixture_support.seen",
         "tests/misc_root_tests/seen_inf_simd_f.sh")
model = next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
              if line.startswith("model name")), platform.machine())
usage = json.loads((root / "benchmark-usage.json").read_text())
if usage["exit_status"] != status or usage["process_peak_rss_bytes"] <= 0:
    raise SystemExit("FAIL: quantized process usage does not match the native benchmark")
report = {"schema": "seen-quantized-simd-performance-v1", "target": f"{platform.system().lower()}-{platform.machine()}",
          "cpu_model": model, "os_release": platform.release(), "compiler_sha256": digest,
          "source_sha256": {path: hashlib.sha256((source_root / path).read_bytes()).hexdigest() for path in paths},
          "command": f"SEEN_REQUIRE_QUANTIZED_SIMD_PERF={int(enforce)} bash tests/misc_root_tests/seen_inf_simd_f.sh",
          "cgroup": group_path, "limits": limits, "warmups": 5, "samples": 30,
          "blocks_per_sample": 128, "ceiling_ppm": 1050000, "enforced": enforce,
          "within_ceiling": within_ceiling, "exit_status": status, "cases": groups,
          "vector_bits_are_requested_ir_store_widths": True, "production_certified": False,
          "process_peak_rss_bytes": usage["process_peak_rss_bytes"], "process_peak_rss_status": usage["process_peak_rss_status"],
          "physical_vram_bytes": None, "physical_vram_status": "not-applicable-cpu-workload"}
(root / "quantized-performance.json").write_text(json.dumps(report, indent=2) + "\n")
PY
rg '^(BENCH:|PASS:|PENDING:)' "$TMP_DIR/00-active.log"
exit "$benchmark_status"
