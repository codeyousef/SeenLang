# Bootstrap System

Seen is self-hosted: a known-good Seen compiler builds the next compiler, then
that compiler builds the compiler again. The rebuild is accepted only when the
new stages verify correctly.

## Stages

1. **Stage 1**: frozen bootstrap compiler.
2. **Stage 2**: current source compiled by Stage 1.
3. **Stage 3**: current source compiled by Stage 2.
4. **Verification**: Stage 2 and Stage 3 outputs must match the expected
   fixed-point checks used by `scripts/safe_rebuild.sh`.

## Key Files

| File | Purpose |
|------|---------|
| `bootstrap/stage1_frozen` | Known-good bootstrap compiler |
| `bootstrap/stage1_frozen.sha256` | Integrity hash for the frozen compiler |
| `compiler_seen/target/seen` | Verified compiler output |
| `target/release/seen` | Release copy of the verified compiler |
| `scripts/safe_rebuild.sh` | Guarded staged rebuild |
| `scripts/seen_prebuild_gates.sh` | Early source/IR prebuild gates |
| `scripts/fix_ir.py` | Frozen-bootstrap IR repair guard for known malformed IR patterns |

## Safe Rebuild

`scripts/safe_rebuild.sh` has three tiers:

| Tier | Purpose | Output |
|------|---------|--------|
| `--tier quick` | Cache-enabled developer rebuild with smoke checks only. | `compiler_seen/target/seen-dev` |
| `--tier verify` | Cache-enabled production rebuild with prebuild gates, smoke checks, and targeted compiler checks before install. | `compiler_seen/target/seen` and `target/release/seen` |
| `--tier full` | Cold staged bootstrap verification with the existing Stage 1/2/3 and recovery semantics. This is still the no-argument default. | `compiler_seen/target/seen`, `target/release/seen`, and a full-release stamp |

Do not run a compiler rebuild without explicit memory limits. A typical guarded
run derives a main cap from current memory and keeps optimizer work capped:

```bash
AVAIL_KB=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)
MAIN_KB=$(( AVAIL_KB * 70 / 100 ))
if [ "$MAIN_KB" -gt 10485760 ]; then MAIN_KB=10485760; fi
ulimit -v "$MAIN_KB"
SEEN_LOW_MEMORY=1 \
SEEN_SKIP_LOW_MEMORY_SHORTCUT=1 \
SEEN_MAIN_VMEM_KB="$MAIN_KB" \
SEEN_OPT_VMEM_KB=2097152 \
SEEN_MEMORY_LIMIT_BYTES="$(( MAIN_KB * 1024 ))" \
./scripts/safe_rebuild.sh
```

The script runs prebuild gates before expensive compiler work unless explicitly
disabled with `SEEN_SKIP_PREBUILD_GATES=1`.

For iterative work, use the same derived caps with a quicker tier:

```bash
SEEN_LOW_MEMORY=1 \
SEEN_MAIN_VMEM_KB="$MAIN_KB" \
SEEN_OPT_VMEM_KB=2097152 \
SEEN_MEMORY_LIMIT_BYTES="$(( MAIN_KB * 1024 ))" \
./scripts/safe_rebuild.sh --tier quick
```

`--clean-cache` explicitly removes `.seen_cache/`, `/tmp/seen_ir_cache`,
`/tmp/seen_thinlto_cache`, and generated-test object caches. Quick and verify
tiers do not clear useful caches during normal rebuilds.

## Build Telemetry

Set `SEEN_TRACE_BUILD=<path>` to write JSONL build events. `SEEN_BUILD_TRACE`
is accepted as a compatibility alias.

```bash
SEEN_TRACE_BUILD=/tmp/seen-build.jsonl ./scripts/safe_rebuild.sh --tier quick
```

The scripts also print a concise timing summary. Use `scripts/perf_gate.sh` to
record and compare capped performance baselines. `scripts/build_perf_gate.sh`
remains a compatibility wrapper for the build suite.

```bash
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --record-baseline --suite build --tier quick
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite build --tier quick
scripts/perf_gate.sh --compare --suite stdlib
scripts/perf_gate.sh --compare --suite runtime
scripts/perf_gate.sh --compare --suite release-lto
scripts/perf_gate.sh --compare --suite packages --version 0.9.2-build
```

Schema-v2 baselines are stored under
`target/seen-build/perf-baselines/<suite>/`. Build traces include guard
state/status and peak RSS fields for guarded commands. Regression failures
include the suite or benchmark name, threshold, observed value, and next action.
Compiler module cache keys also include the active compiler binary hash, so a
warm quick/verify rebuild keeps valid objects only when they were produced by a
compatible compiler.

Benchmark suites use the verified `compiler_seen/target/seen` binary by
default. Set `SEEN_BENCH_USE_DEV=1` when the intent is to measure the current
quick-tier `compiler_seen/target/seen-dev` output instead. Production benchmark
scripts preserve warm caches unless `SEEN_BENCH_COLD_CACHE=1` is set.

## Worker Budgets

The rebuild script derives `SEEN_JOBS` and `SEEN_OPT_JOBS` from CPU count and
the memory caps when the variables are not supplied. The compiler also accepts
`--jobs <n>` and `--opt-jobs <n>`. `--no-fork` remains the explicit serial
escape hatch.

Quick and verify tiers choose a trusted builder before doing frozen-bootstrap
startup/hash checks or creating the bootstrap source overlay. Those frozen-only
steps still run for full cold verification and for quick/verify fallback to a
frozen compiler. Quick/verify smoke compiles use a signature-keyed cache, but
the smoke executable is still run every time.

## Prebuild Gates

The prebuild gate catches failures that used to appear late in Stage 2/Stage 3:

- compiler-codegen ABI boundary drift
- missing imported/seeded compiler modules
- compiler import cycles
- malformed frozen-bootstrap IR patterns repaired by `fix_ir.py`
- stale package/runtime artifact assumptions

Run it directly when changing compiler source or bootstrap scripts:

```bash
scripts/seen_prebuild_gates.sh
```

## Package Artifacts During Bootstrap

Package prebuild artifacts contain:

- `Seen.pkg.toml`
- `objects.tsv`
- `interface.index.tsv`
- interface/source files under the artifact root
- prebuilt object files

During dependent builds, the compiler loads declarations from the artifact
interface/index, links listed objects, and skips code generation for modules
provided by the artifact.

## Manual Smoke Checks

When a rebuild succeeds, verify the fresh compiler can compile a minimal program
before replacing any system-wide binary:

```bash
cat > /tmp/seen_hello.seen <<'SEEN'
fun main() {
    println("hello")
}
SEEN
compiler_seen/target/seen compile /tmp/seen_hello.seen /tmp/seen_hello
/tmp/seen_hello
```

## Updating a System Binary

Only copy a compiler into a PATH location after Stage 2/Stage 3 verification and
smoke tests pass. Then compare hashes:

```bash
sha256sum compiler_seen/target/seen target/release/seen "$(command -v seen)"
```

## Emergency Recovery

If the working compiler is broken, use the frozen compiler and the guarded
rebuild scripts instead of retrying uncapped builds:

```bash
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen recovery_compiler
```

If a rebuild fails, inspect the first concrete failing log/artifact and fix that
cause before retrying.

## Related

- [Compiler Architecture](compiler-architecture.md)
- [Known Limitations](known-limitations.md)
