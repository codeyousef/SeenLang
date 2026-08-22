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
| `scripts/fix_ir.py` | Frozen Stage-1 IR compatibility adapter; never a production compiler path |

## Safe Rebuild

The temporary bootstrap source view is byte-identical to the checkout. It can
replace the live compatibility manifest with the immutable manifest paired to
the frozen compiler and can provide a symlink-free package layout, but it does
not edit Seen source. Every copied source file is checked with `cmp`; a mismatch
fails the rebuild. Production source rewriting and documentation-body stripping
are forbidden.

`scripts/safe_rebuild.sh` has three tiers:

| Tier | Purpose | Output |
|------|---------|--------|
| `--tier quick` | Cache-enabled developer rebuild with smoke checks only. | `compiler_seen/target/seen-dev` |
| `--tier verify` | Cache-enabled production rebuild with prebuild gates, smoke checks, and targeted compiler checks before install. | `compiler_seen/target/seen` and `target/release/seen` |
| `--tier full` | Cold staged bootstrap verification with the existing Stage 1/2/3 and recovery semantics. This is still the no-argument default. | `compiler_seen/target/seen`, `target/release/seen`, and a full-release stamp |

### Hard-containment contract

Do not run a compiler, rebuild, package-helper build, optimizer, linker, or
build-capable test without an aggregate kernel-enforced memory boundary around
the whole process tree. A per-process `ulimit` is only a secondary defense: it
does not stop many children from exhausting the host together.

On Linux, a supported rebuild must create one transient user-systemd scope
around the entire post-namespace rebuild and read back all of these controls
before starting any compiler or helper build:

- `memory.max` is a positive value no greater than the requested aggregate cap;
- `memory.swap.max` is exactly `0`;
- `pids.max` is a positive value no greater than the requested `TasksMax`.

The automatic aggregate cap is the minimum of 60% of total memory and currently
available memory after retaining a 10%-of-total system reserve. The main
compiler VMEM and allocation limits equal that live-derived cap; a caller may
request a smaller value but never a larger one. The optimizer's secondary
per-process limit is the minimum of 10% of total memory, half the aggregate
cap, and 2 GiB.
The rebuild also forces `SEEN_JOBS=1`, `SEEN_OPT_JOBS=1`, and `TasksMax` no
greater than 24. `ulimit -v` and the userspace RSS observer run inside that
scope as early-warning and per-process defenses; neither replaces the cgroup.

Optional host compatibility runners must fit inside the same task ceiling.
The Wine-backed atomic-I/O fixture disables Wine desktop, service, and menu
processes that are unrelated to its Win32 file-API contract, executes the
Windows binary three times, and waits for `wineserver` before cleanup. Do not
raise `pids.max` or hide an installed compatibility runner to make that test
pass.

If the user systemd manager, cgroup v2 controls, or limit read-back is
unavailable, the rebuild must fail before compiler work begins. Polling-only
containment and an unverified environment marker are not supported fallbacks.
This section defines the required policy; the read-back reported by each run is
the evidence that the particular checkout and host enforced it.

Source rebuilds of Seen 0.10 also require Go 1.26 or newer to build the
version-matched `seen-pkg` helper. Set `SEEN_GO` when that executable is not the
default `go` on `PATH`. Binary releases already include the helper beside the
compiler and do not require Go.

Use the guarded rebuild entry point and spell the tier explicitly:

```bash
SEEN_LOW_MEMORY=1 \
SEEN_JOBS=1 \
SEEN_OPT_JOBS=1 \
./scripts/safe_rebuild.sh --tier quick
```

The command is permitted to continue only after it reports the read-back
verified `MemoryMax`, zero swap allowance, and task limit. Do not wrap it in an
ad hoc `ulimit` block and mistake that for aggregate containment.

The verify/full paths run their required prebuild gates inside the same hard
scope before expensive compiler work. Do not disable them for a routine
rebuild.

Required pull-request CI uses the same boundary through
`scripts/run_ci_required.sh`. The wrapper derives a cap from current memory,
creates a project-local artifact directory, enters the hard scope with a
10,800-second deadline, and then invokes `scripts/ci_required.sh`. The inner gate
must successfully run `scripts/run_in_hard_memory_scope.sh --verify-only`
before any required checks proceed. The final required gate performs the
clean-checkout full rebuild, canonical Seen test, fuzz smoke, and deterministic
package certification described by `seen-gate0-certification-v1`. Its versioned contract is
`docs/architecture/ci-containment.json`; unsupported hosts fail before running
the gate.

All rebuild scratch files default to the Git-ignored
`.seen/agent-tools/safe-rebuild/` directory in the checkout. Set
`SEEN_ARTIFACT_ROOT` to choose a different location, but it must still be an
ignored directory inside the repository and must not traverse symbolic links.
Validate the location and frozen-bootstrap mapping without starting a build:

```bash
./scripts/safe_rebuild.sh --artifact-preflight
```

The frozen compiler predates the artifact-root setting and uses absolute
temporary paths internally. On Linux, `safe_rebuild.sh` uses Bubblewrap to map
those paths onto its project-local per-run directory, then enters the transient
scope without losing that mount namespace. The host temporary filesystem is
not written. The script fails closed if either the private mapping or the hard
scope is unavailable. `SEEN_ALLOW_SYSTEM_TMP=1` is an explicit compatibility
escape hatch for other hosts, and must not be set when project-local artifacts
are a requirement.

`scripts/run_with_project_artifacts.sh` preserves command arguments, creates a
unique ignored directory under `.seen/agent-tools/<scope>/`, sets `TMPDIR` and
`SEEN_ARTIFACT_ROOT`, and maps that directory onto `/tmp` only inside the
command's private Linux mount namespace. It cleans the unique directory after
the command. Add
`--keep-on-failure` after the scope to retain a failed run for inspection. It
fails closed when Bubblewrap or the private mapping is unavailable.

Artifact isolation is not memory containment. Until a standalone entry point
can place a direct compiler, prebuild, performance, or package-helper command in
the same read-back-verified scope, do not invoke those build-capable paths
directly or use the artifact wrapper as a substitute. Drive them through the
appropriate `safe_rebuild.sh` tier instead.

For verification or a cold fixed-point rebuild, replace the final tier with
`--tier verify` or `--tier full` while retaining low-memory mode and the serial
worker settings. Always spell the tier explicitly in automation, even though no
argument currently means `full`.
`SEEN_SKIP_LOW_MEMORY_SHORTCUT=1` deliberately disables a protective shortcut;
do not set it for routine work.

```bash
./scripts/safe_rebuild.sh --help
```

No equally hard rebuild scope is currently documented for macOS or for Linux
sessions without a working user systemd manager and writable cgroup v2
controls. Rebuilds on those hosts must fail closed until an equivalent,
read-back-verified aggregate boundary is implemented.

`--clean-cache` explicitly removes `.seen_cache/` and the rebuild's isolated IR,
ThinLTO, and generated-test object caches. Quick and verify tiers do not clear
useful caches during normal rebuilds.

## Build Telemetry

Set `SEEN_TRACE_BUILD=<path>` to write JSONL build events. `SEEN_BUILD_TRACE`
is accepted as a compatibility alias.

Add `SEEN_TRACE_BUILD=.seen/agent-tools/seen-build.jsonl` to the guarded rebuild
invocation above; do not start a second rebuild just to collect telemetry.

The scripts also print a concise timing summary. `scripts/perf_gate.sh` records
and compares performance baselines, and `scripts/build_perf_gate.sh` remains a
compatibility wrapper for the build suite. Both are build-capable entry points;
do not run them standalone until they provide the same whole-command verified
scope. A per-benchmark userspace guard does not satisfy this requirement.

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

The rebuild requires `SEEN_JOBS=1` and `SEEN_OPT_JOBS=1`; values other than one
must be rejected. A candidate builder is usable only when capability discovery
proves that it supports both `--jobs` and `--opt-jobs`, supports `--no-fork`, or
is placed behind an explicit serializer that bounds its descendants. Passing a
flag to a permissive legacy CLI is not proof that the flag was honored.

Quick and verify tiers choose a trusted, capability-proven builder before doing
frozen-bootstrap startup/hash checks or creating the bootstrap source overlay.
A frozen or legacy builder that advertises none of the bounded-worker controls
must be rejected instead of being used as an automatic fallback. Quick/verify
smoke compiles use a signature-keyed cache, but the smoke executable is still
run every time.

## Prebuild Gates

The prebuild gate catches failures that used to appear late in Stage 2/Stage 3:

- compiler-codegen ABI boundary drift
- missing imported/seeded compiler modules
- compiler import cycles
- frozen Stage-1 IR compatibility patterns handled only by the explicit
  bootstrap adapter
- byte-identical bootstrap source view enforcement
- unmodified current-compiler IR verification before optimizer/object use
- stale package/runtime artifact assumptions

The verify and full rebuild tiers run the gate inside their aggregate hard
scope. Do not run `scripts/seen_prebuild_gates.sh` directly until it has a
standalone entry point with the same read-back-verified containment.

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

## Smoke Checks

Quick, verify, and full tiers compile and execute their required smoke fixtures
inside the rebuild's aggregate hard scope. Do not repeat the smoke with a direct
compiler invocation outside that scope. Before replacing a system-wide binary,
require the verify/full smoke evidence from the fresh compiler.

## Updating a System Binary

Only copy a compiler into a PATH location after Stage 2/Stage 3 verification and
smoke tests pass. Then compare hashes:

```bash
sha256sum compiler_seen/target/seen target/release/seen "$(command -v seen)"
```

## Emergency Recovery

If the working compiler is broken, use the frozen compiler and the guarded
rebuild script instead of invoking the frozen compiler directly or retrying
uncapped builds:

```bash
./scripts/safe_rebuild.sh --tier full
```

If a rebuild fails, inspect the first concrete failing log/artifact and fix that
cause before retrying.

## Related

- [Compiler Architecture](compiler-architecture.md)
- [Known Limitations](known-limitations.md)
