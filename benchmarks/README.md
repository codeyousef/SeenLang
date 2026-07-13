# Seen Benchmark Suite

## Maintained Gates

Use `scripts/perf_gate.sh` from the repository root for maintained benchmark
checks. The gate fixtures in `benchmarks/gates/` are valid Seen programs and are
the source of truth for 0.9.4 performance acceptance.

```bash
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite stdlib
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite runtime
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite build --tier quick
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite release-lto
```

For reproducible local runs, set explicit caps derived from the current host:

```bash
SEEN_LOW_MEMORY=1 \
SEEN_MEMORY_LIMIT_BYTES=17179869184 \
SEEN_MAIN_VMEM_KB=16777216 \
SEEN_OPT_VMEM_KB=2097152 \
scripts/perf_gate.sh --compare --suite stdlib
```

## Recording Baselines

Record baselines only after confirming the benchmark is valid and the change is
intentional:

```bash
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --record-baseline --suite stdlib
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --record-baseline --suite runtime
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --record-baseline --suite build --tier quick
```

Baselines are stored under `target/seen-build/perf-baselines/`. Results and
temporary binaries are written under `target/seen-build/perf-results/`.

## Gate Coverage

- `stdlib`: collection behavior, byte buffers, string/JSON, math, sort/search
- `runtime`: allocation-budget behavior
- `build`: tiered rebuild duration, peak memory, cache hit rate, compiler size
- `release-lto`: default merged LTO plus explicit `--no-merged-release-lto`
- `packages`: release artifact staging and reuse

## Legacy Suites

The older PowerShell and cross-language benchmark harnesses are preserved for
experiments and future migration. Some legacy `.seen` files outside
`benchmarks/gates/` still use early syntax or pseudo-code and are not accepted
as optimization evidence until converted.

When migrating a legacy benchmark, keep the default gate small and reliable.
Long production comparisons should remain opt-in so everyday compiler rebuilds
stay fast and bounded.
