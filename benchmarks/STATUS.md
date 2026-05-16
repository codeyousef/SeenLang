# Benchmark Suite Status

## Canonical 0.9.0 Gates

The maintained performance gates live in `benchmarks/gates/` and are driven by
`scripts/perf_gate.sh` from the repository root. These fixtures are valid Seen
programs and are the only benchmark inputs currently used for optimization
acceptance.

Covered gate suites:

- `stdlib`: collections, byte buffers, string/JSON, math, sort/search
- `runtime`: allocation-budget behavior
- `build`: quick/verify/full rebuild timing, cache rate, peak memory
- `release-lto`: default merged LTO and explicit opt-out behavior
- `packages`: release-package staging and artifact reuse

Run gates with explicit memory caps, following the repository build policy:

```bash
SEEN_LOW_MEMORY=1 SEEN_MEMORY_LIMIT_BYTES=17179869184 \
  SEEN_MAIN_VMEM_KB=16777216 SEEN_OPT_VMEM_KB=2097152 \
  scripts/perf_gate.sh --compare --suite stdlib
```

Record new baselines only after an intentional improvement or accepted
environment change:

```bash
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --record-baseline --suite stdlib
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --record-baseline --suite runtime
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --record-baseline --suite build --tier quick
```

## Legacy Material

Older benchmark directories remain useful as migration material, but they are
not acceptance gates yet. Some files still use early Seen syntax, pseudo-code,
or old cross-language harness assumptions.

Legacy directories:

- `benchmarks/seen/`
- `benchmarks/seen_benchmarks/`
- `benchmarks/microbenchmarks/`
- `benchmarks/production/`
- `benchmarks/real_world/`
- `benchmarks/systems/`
- `benchmarks/harness/`

Do not cite results from those directories as optimization evidence until the
fixtures compile with the current compiler and are added to `perf_gate.sh`.

## Conversion Rules

When converting a legacy benchmark:

- Keep it deterministic and self-contained.
- Avoid benchmark code that clears caches unless the suite is explicitly
  measuring cold-cache behavior.
- Add the fixture under `benchmarks/gates/` for small acceptance checks, or keep
  longer comparisons outside the default gate.
- Add a baseline under `target/seen-build/perf-baselines/` by running
  `scripts/perf_gate.sh --record-baseline`.
- Make failures report the benchmark, threshold, observed value, and next
  action.

## Current Acceptance Path

Use this order for performance work:

```bash
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite stdlib
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite runtime
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite build --tier quick
SEEN_LOW_MEMORY=1 scripts/perf_gate.sh --compare --suite release-lto
SEEN_LOW_MEMORY=1 scripts/safe_rebuild.sh --tier verify
```

Release uploads and package checks still require the release scripts and their
own verification stamps.
