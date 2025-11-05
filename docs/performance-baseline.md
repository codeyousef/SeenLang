# Seen Performance Baseline Harness

The `perf_baseline` tool automates the performance sampling requested in the
MVP pre-bootstrap plan. It records wall clock latency, peak RSS, and (for
compile tasks) artifact sizes so that we can track regressions over time.

## Quick start

```bash
# Run the default suite defined in scripts/perf_baseline.toml
cargo run -p perf_baseline -- --config scripts/perf_baseline.toml

# Compare against an existing baseline (fails if mean runtime regresses >5%)
cargo run -p perf_baseline -- \
  --config scripts/perf_baseline.toml \
  --baseline scripts/perf_baseline_report.json
```

This generates a JSON report (`scripts/perf_baseline_report.json`) and prints a
summary table to stdout.

## Configuration format

Tasks are declared in TOML:

```toml
[[tasks]]
name = "interp:simple_test"
kind = "micro"        # micro, macro, or compile
command = ["cargo", "run", "-p", "seen_cli", "--", "run", "simple_test.seen"]
working_dir = "."
warmups = 1            # optional (default: 1)
runs = 3               # optional (default: 3)
clean_before = false   # compile tasks only
artifacts = ["target/release/seen_cli"]
```

- `warmups` are executed first and ignored in the final statistics.
- `runs` are measured; the harness reports per-run data and aggregated mean,
  median, and p95 timings along with the maximum RSS observed.
- `clean_before` triggers a `cargo clean` before the first measured run so we
  capture both cold and incremental build behaviour.
- `threshold_pct` (default 5) controls the allowed mean-runtime regression when
  comparing against a baseline via `--baseline`.
- `artifacts` (optional) list files whose sizes should be recorded after the
  task completes.

Environment variables can be injected per task via:

```toml
[tasks.env]
SEEN_PROFILE = "deterministic"
```

## Metrics collected

For each measured run the harness captures:

- `duration_ms` – wall clock time in milliseconds.
- `peak_memory_kib` – peak RSS sampled via `sysinfo` (KiB).
- `status` – process exit code.

Aggregate statistics include the arithmetic mean, median, and p95 latencies as
well as the maximum peak RSS observed.

## Baseline suite

`scripts/perf_baseline.toml` seeds the baseline suite with:

1. A micro benchmark that interprets `simple_test.seen` via `seen_cli`.
2. A macro benchmark that runs the `simple_showcase.seen` example through the
   interpreter.
3. A compile benchmark that measures a clean and incremental
   `cargo build -p seen_cli --release`, also recording the resulting binary size.

These targets can be extended as new workloads become relevant. CI pipelines can
invoke the harness and compare the emitted JSON against stored baselines to gate
regressions.

## CI integration

`ci.yml` now includes a **Performance Baseline** job that:

1. Checks out the workspace and installs the stable toolchain.
2. Re-uses the shared cargo cache so the release compilation step is not overly
   expensive.
3. Executes `cargo run -p perf_baseline -- --config scripts/perf_baseline.toml \
   --baseline scripts/perf_baseline_report.json --output target/perf/latest.json \
   --json-stdout`.
4. Publishes `target/perf/latest.json` as the `perf-baseline-report` workflow
   artifact.

The run fails automatically if the recorded mean latency for any task exceeds
its configured threshold relative to the baseline JSON.

For a higher-level summary, the new `docs/performance-dashboard.md` file
renders the same baseline metrics into a parity table that we can extend with
C++ measurements once they are available.
