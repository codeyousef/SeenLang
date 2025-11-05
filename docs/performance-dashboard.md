# Seen Performance Dashboard (Rust vs. C++ Targets)

This dashboard converts the seeded `perf_baseline` report into a human-readable
summary so the MVP team can track how the Rust implementation compares against
the longer-term C++ parity goals surfaced in the research documents.

| Task | Kind | Rust mean (ms) | Rust p95 (ms) | Rust max RSS (MiB) | Artifact size (MiB) | C++ parity target (ms) |
| ---- | ---- | -------------- | ------------- | ------------------ | ------------------- | ---------------------- |
| interp:simple_test | micro | 344.56 | 537.82 | 61 224 | — | TBD |
| build:simple_showcase | macro | 244.34 | 258.39 | 61 116 | — | TBD |
| compile:seen_cli_release | compile | 39 690.81 | 79 122.64 | 63 256 | 4.01 | TBD |

**Notes**

- Numbers come from `scripts/perf_baseline_report.json` (generated
  2025-11-05T10:29:27Z). The harness reports maximum resident set size in KiB;
  values above are converted to MiB for readability.
- C++ reference runs are still pending; once collected they can slot into the
  final column so deltas and required speedups are visible at a glance.
- The raw JSON emitted on CI is uploaded as the `perf-baseline-report`
  artifact; the `perf_baseline` tool also supports `--json-stdout` for quick
  inspection during local profiling passes.
