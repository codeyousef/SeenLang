# Testing

## Fixture execution oracles

The runtime-A metadata, IOCP completion and ARM dispatch harnesses use native
assertions and exact process statuses as their functional oracles, rather than
matching human-readable PASS sentences. Stable diagnostic codes, structured
capability classifications, coverage thresholds and benchmark sample contracts
remain checked separately.

## Complete hosted acceptance

Ordinary `scripts/run_ci_required.sh` and full rebuilds still execute the complete
acceptance surface. Hosted CI first exports a fresh portable Stage3 candidate
after both bootstrap stages and hello checks. This candidate is explicitly
unverified: it does not install a production compiler or create a release stamp.

The canonical acceptance runner supports four complete groups: `core`,
`io-simd`, `gpu-emitter` and `gpu-runtime-async`. Its default `--group all`
preserves local full/verify behavior. Hosted workers run these groups on separate
machines with identical candidate bytes, same-commit source, fresh live kernel
containment, zero swap and serial compiler workers. No profile, native assertion,
fuzz duration, coverage threshold or benchmark sample contract is removed.

The final required job rejects missing, failed or skipped dependencies and
validates all four source/candidate-bound completion receipts before installation.
It then executes the installed compiler's Gate 0 checks and writes the full
release stamp. Only this job can produce the exact-commit release toolchain.
Candidate transport is bounded and admitted before publication; archive links,
traversal, duplicate entries and substituted source or binaries fail closed.

The configured dependency-path budgets are 120 minutes for bootstrap,
195 minutes for the acceptance matrix and 30 minutes for finalization:
345 minutes total, matching the existing 20,700-second hard CI bound.

## Interrupted runtime Gate C diagnostics

`seen_p0_accel_gate_c.sh` publishes `active-child.json` before snapshot,
execution and observation work, with the child name, stage, shell-elapsed
seconds and nullable actual child exit status. The parent log also records
each transition and its shell-elapsed time, so cumulative suite cost can be
distinguished from a child that stops progressing.
Child and observation status files begin as `incomplete`;
only completed processes replace them with numeric statuses. Marker, status
and checks updates are atomic publications, not power-loss durability claims.
An aggregate killed before its EXIT trap still leaves these breadcrumbs.

Clean-checkout certification enables the existing build trace for its full
rebuild. `safe_rebuild.sh` rebases the trace into its ignored work directory as
`safe-rebuild.trace.jsonl`; it does not create a file named `1` in the checkout.
Compiled fixture helpers add compile/run begin events and completion events
with the actual exit status, fixture name and profile, alongside compiler phase
events. These use Unix wall-clock milliseconds, not benchmark-quality monotonic
timings. A begin without a completion identifies unfinished work, not its cause.
Fixture events record no command arguments, environment dumps, IR or native payloads.

`scripts/collect_verification_logs.py` retains these fixed-depth, allowlisted
diagnostics from unfinished `p0-accel-c.*` directories. Active child identity,
log and status outrank older build tails; checks and snapshot/observation
metadata follow within the unchanged 128-file, 1 MiB-per-file, 32 MiB-total
and 16,384-discovery-entry bounds. It never descends into child artifacts or
copies IR/native binaries. Directory and file opens reject symlinks; nonregular
files cannot block collection. The index marks truncation, observed mutation,
incomplete JSON and sources that disappear before reading.

The same bounded collector retains `safe-rebuild.trace.jsonl` with the top-level
build tails. Its final JSONL record may be partial after interruption or tail
truncation; a missing completion must not be interpreted as a successful compile.

Retained files are independent tails, not an atomic gate snapshot or proof of
success. A missing or malformed marker cannot identify an active child.
Timeouts and sanitizer failures remain failures; neither diagnostic retention
nor an earlier successful child certifies an unfinished aggregate.

## Gate B native emitter fixture provisioning

`tests/misc_root_tests/seen_p0_accel_gate_b.sh` owns a private pool below its
current attested artifact directory. The producer uses the unchanged generic
`compile_fixture` path with `--no-cache --frozen`: `text_layout_contract.seen`
and `integrity_contract.seen`, each in `fast`, `release` (ThinLTO), `undefined`
and `address`, for eight actual builds. The original 18 children still enter
their own attested scopes. Exactly the six PTX/AMDGPU/Metal A/B text-program
calls and four SPIR-V/PTX/AMDGPU/Metal C integrity-program calls use the pool
(40 profile deliveries). All runtime loops, backend/case arguments, goldens,
typed rejections, SDK checks and other source builds remain independent.
Without the pool selector, the ten leaves use their original fresh compilation.

The internal `scripts/emitter_fixture_support.sh` API is
`prepare_emitter_fixture_pool POOL`, `compile_emitter_fixture PROFILE NAME SOURCE`
and `compile_emitter_case PROFILE NAME SOURCE [runtime args...]`. Sourcing it
installs no trap; leaves source it after the generic fixture helper. Only B
provisions and exports `SEEN_EMITTER_FIXTURE_POOL`, after a valid seal.
The Python helper provides `fingerprint`, `seal` and `clone`, all taking
`--root`, `--compiler`, `--pool`; `clone` additionally requires `--source`,
`--profile`, `--destination` (the executable prefix).

The seal binds the actual compiler path/bytes, existing bounded HEAD/dirty
source identity, compiler-source/standard-library/runtime/language dependency bytes, exact
program/profile/flags/default x86-64 CPU, selected native tool and bounded-wrapper
bytes, artifact-root/pool ownership, cgroup and mount namespace identity.
The global mount table is not a compilation input: unrelated mount churn must
not invalidate unchanged SDK/source bytes or the owned artifact bindings.
Semantic environment values are hashed, not published; only leaf scope,
`SHLVL`, `_` and the pool selector are omitted, and duplicate PATH components
are removed without changing search order. A changed dependency or context
rejects the pool; it never silently falls back to recompilation or cross-run
reuse. B retains producer and adverse logs/statuses plus the sealed manifest.
These are build/admission evidence, not physical GPU certification.
Compiler-owned runtime object/signature caches are derived outputs, not stable
source identities: the catalog excludes only the named legacy runtime object
outputs and normal generated cache/target directories, while binding their
authoritative source/header/toolchain/profile inputs. Unrecognized native
dependency files are not blanket-excluded by extension.
System SDK directories are not assumed immutable merely because the mount
namespace is unchanged. Each admission discovers C/C++ search roots using the
selected bounded Clang wrappers with empty preprocessing, resource-directory
and startup/support-file queries. It streams the actual include, Clang
resource/sanitizer and selected startup/support bytes, plus resolved native-tool
shared dependencies and loader obtained with trusted `ldd`. Explicit include
and library environment roots are included. Canonical aliases and directory
cycles are bounded; missing required libraries, unsupported dependency records
or catalog overflow reject admission. Only aggregate hashes/counts enter the
manifest: at most 100,000 unique SDK files, 200,000 traversal entries, 4 GiB
total bytes and 512 MiB per file, using 1 MiB streaming reads. Discovery output
is capped at 256 KiB per 30-second tool call. No SDK tree is copied, mounted
differently or presumed safe after an unobserved mutation.
Inspector admission checks protected paths and effective write permissions,
not host UID numbers that a user namespace may remap. Relocatable startup
objects are hashed without asking the dynamic loader to inspect them. Loader
stdout records and stderr diagnostics stay separate; diagnostics are retained
in the SDK identity with a 32 KiB aggregate ceiling, not discarded or treated
as dependency records. Unexpected failures or unresolved dynamic inspection reject.

Every clone validates all eight slots and writes the producer's actual
successful compile status last. Executables are read-only same-filesystem
hardlinks; `.compile.log`, `.log`, `.compile.status` and
`.compile.origin.json` are independent consumer metadata, explicitly identifying
same-run reuse rather than a new compilation. The pool is limited to eight
native blobs, 128 MiB each and 512 MiB total, 8 MiB per producer log and 256 KiB
per context/manifest. Only successful producer-owned module-IR directories are
removed; failed diagnostics remain. Publication guarantees atomic visibility,
not power-loss durability. No memory/task/deadline policy is relaxed.

Before children, `emitter_fixture_pool_contract.py` uses the real sealed pool
for stale-context, source/compiler/profile, ownership, completeness and damage
refusals. Its two temporary successful clone controls are separate from the
40 leaf deliveries and do not execute a fixture or Seen compiler/build; native
SDK identity probes still run. Measure actual
baseline/aggregate wall time separately; no speedup follows from build-count
reduction alone.

## Official SPIR-V runtime validator

The pinned Ubuntu `spirv-tools` package includes the command-line tools and
`libSPIRV-Tools.a`, but not the shared C ABI required by runtime artifact
validation. `scripts/install_gpu_spirv_validator.py` verifies the archive hash
in `docs/architecture/spirv-toolchain.json` and links only those official vendor
bytes. It requires the platform-only capped entrypoint; the linker is serial
and has a finite process-group deadline. The installer assembles a real Vulkan
1.2 compute module and requires the loaded vendor C ABI to accept it and reject
a corrupted magic word, destroying both contexts and diagnostics.

With the pinned package installed, a non-root local installation is:

```sh
mkdir -p .seen/toolchains/spirv-validator
SEEN_MEMORY_GUARD_RSS_KB=2097152 \
  bash scripts/run_capped_regression.sh --platform seen-spirv-validator-sdk -- \
  python3 scripts/install_gpu_spirv_validator.py \
  "$PWD/.seen/toolchains/spirv-validator/libSPIRV-Tools-shared.so"
```

The wrapper refuses a cap above the live-memory-derived ceiling and enforces
zero swap and bounded tasks. `--archive PATH` accepts only an exact matching
archive, not a different validator implementation. Output is limited to an
8 MiB archive and 8 MiB shared image. Archive, linker and validation refusals
leave the previous destination unchanged. Publication is atomic, not
power-loss durable.

Applications must expose the installed directory through their normal dynamic
loader configuration (or a command-scoped `LD_LIBRARY_PATH`). Required CI
installs the library through the system loader cache instead of exporting an
SDK search directory into every CPU-only process. CPU-only import/build paths
still never call this validator. The installer proves vendor C ABI admission,
not GPU execution; runtime E–H retain their independent native correctness,
sanitizer, ownership and failure checks.

## Persistent async mixed-workload soak

The manual `scripts/run_async_stress_soak.sh` defaults to exactly 86,400 seconds
on a discovered NVIDIA RTX 4090. It is not an automatically discovered test.
`tests/misc_root_tests/seen_async_stress_soak_slow.sh` instead performs a real
two-second software-reference smoke plus malformed/range/device-class cases.
Selecting slow tests must never accidentally start the 24-hour scenario.

For the manual run, select the verified current compiler through the normal
rebuild workflow, derive a finite physical-memory cap from live RAM and
containing cgroup headroom, and supply a separate finite long-run scope budget:

```bash
: "${SEEN_GUARD_RSS_KB:?Set a finite current-memory-derived cap in KiB}"
export SEEN_GUARD_RSS_KB
SEEN_GUARD_TASKS_MAX=24 SEEN_JOBS=1 SEEN_OPT_JOBS=1 \
SEEN_MEMORY_GUARD_TIMEOUT_SECS=87000 SEEN_TIER_TIMEOUT_SECS=87000 \
  bash scripts/safe_rebuild.sh \
    --regression-script scripts/run_async_stress_soak.sh
```

This leaves ordinary CI, one-hour `seen test` and short-fixture deadlines
unchanged. The manual shell/native CLI accepts `--seconds N` / positional
`SECONDS` only in `1..86400`, with explicit `rtx4090` or `software-reference`
device class. Long runs require a caller budget covering duration plus bounded
preparation/drain. No hardware fallback is implicit. Hardware execution rejects
inherited loader/fault-layer interposition, selects the actual NVIDIA ICD and
checks device kind, vendor, name and matching discovery/capture/open UUIDs.

The native program is
`tests/production_gpu_runtime/async_stress_soak_contract.seen`; its five operands
are `SECONDS IO_PATH SHADER_SPV REFLECTION_JSON DEVICE_CLASS`. It keeps two CPU
and two readiness workers, the direct-I/O queue/file, and creator-confined GPU
device/context/allocator/stream/pipeline/buffers alive through five complete
mixed warmups, thirty actual raw samples, and the requested monotonic interval.
Every completed round checks 39 CPU executions and 47 destructors including
nested and cancelled work; two successful I/O completions; a real two-kernel GPU
graph with all 37 intermediate/output wrap-u32 oracle values; and a post-oracle
GPU-to-CPU readiness callback. No GPU handle is sent to worker tasks. Retired
graph owners close before oracle mapping; persistent GPU owners remain live.

The reused direct-I/O file stays exactly 8 KiB. Managed allocation, reserved
storage, workers, synchronization and readiness handles are checked against
warmed baselines each round, then resources drain to the original baseline.
Only the independently measured UUID/sample/report owner remains during the
final report; its release is also checked before successful exit.

`scripts/monitor_async_stress_soak.py` observes the exact native argv without a
shell, validates duplicate-free bounded `seen-async-stress-soak-v1` records,
counter relationships, monotonic duration and actual native exit, and owns
finite TERM/KILL/reap cleanup of its child process group. Native minute
heartbeats accompany real `/proc` descriptor/thread/child/RSS/HWM observations.
Baseline/peak/last are observed values; no post-reap zeroes are invented.
Logs and metrics each stop at 2 MiB, with 4 KiB records and one artifact
directory rather than per-iteration files. Successful compiler IR scratch is
removed before timing; failed compile diagnostics remain. The printed artifact
directory contains native `.log`, `.metrics.jsonl`, compile/run statuses,
shader/reflection evidence and bounded `build-inputs.json` / `build-receipt.json`.
The receipts reuse the gate's source/compiler/cgroup identity, reject source
or compiler changes during preparation, and bind actual native/shader/reflection
hashes before runtime. Remove owned executables/projects after retaining the
bounded evidence required for review.

The monitor reports `physical_24h_observed:true` only after an actual successful
86,400-second RTX 4090 run. Short hardware runs and software-reference runs do
not establish that result. Neither this component scenario nor its raw samples
prove a designated performance regression threshold, scaling, TSan startup,
production GPU certification, green required CI or merged-main acceptance.
The original five-profile short stress contract remains separately required.

An actual 24-hour hardware soak has passed. That result closes the physical
duration observation only; the independent sanitizer, platform and complete
milestone acceptance requirements above remain nonwaived.

## Seen source coverage

On Linux x86-64 with LLVM 20, `seen test . --filter <case> --profile coverage
--jobs 1` emits counters and source mappings for Seen function entries and
executable statements, alongside linked native runtime coverage. Structured
conditionals, match patterns and guards, loop tests and short-circuit decisions record both outcomes.
Counters use atomic updates so concurrent executor work contributes without
lost increments. Anonymous callbacks inherit their containing statement's
source location. Coverage maps refer to source locations, not GPU device
execution; running a compiler or GPU source emitter does not execute a kernel.

The compiler owns LLVM profile identities and mappings in Seen. It uses bounded
function, region, counter and filename budgets and rejects malformed filenames
or exhausted budgets. Native LLVM tools merge profiles and render reports.
The module cache includes the selected compiler and coverage mode.
Use matching LLVM profile producers and consumers: an incompatible raw-profile
format is an infrastructure failure, not a coverage result. The shared fixture
export helper merges and exports serially, with the optimizer VMEM ceiling and
a hard command timeout in addition to the calling harness's aggregate scope.

`run_test_instrumentation_acceptance.sh` retains text and JSON coverage exports.
The instrumented host exercises the typed SPIR-V CLI, and the native GPU probe
executes the Seen frontend and emitter directly. The probe checks cleanup and
retains a SPIR-V artifact with its reflection; no GPU execution is claimed.
The evidence checker requires executed Seen compiler-host and GPU-emitter source
rows, an executed Seen test row, and executed native runtime rows. A C-only
report or a `TOTAL` marker cannot satisfy Seen coverage acceptance. Percentage
gates must select the issue's applicable source files explicitly; the aggregate
of every linked library module is not a measure of a single leaf's tests.

The executor, pool, reactor, streams/process/DNS, task-policy and synchronization contracts enforce aggregate 90%
line and 80% branch coverage over their declared implementation files. Their
adverse tests assert exact error codes, partial progress, retained ownership
on failed admission, and allocation and native-handle cleanup.
The reactor also checks loopback byte transfer and EOF, context snapshot isolation,
stale registrations, bounded wake leases, and indexed deadline ordering against
an independent reference model over 20,000 insertion/removal operations.
The streams/process/DNS contract separately requires 90% branch coverage for
the DNS wire parser and process argument validators. A structured CNAME/IPv6 corpus supplements the seed-1101
60-second mutation fuzzer. The DNS integration test exchanges real loopback UDP
datagrams using the existing socket ABI; query/answer policy stays in Seen.
Process tests own, terminate and reap every child, including unclaimed spawn jobs.

## Leak and soak resource providers

TEST-002D defines `seen-test-leak-soak-v1`. A canonical snapshot registers
bounded providers for allocations, file descriptors, threads, child processes,
mapped windows, asynchronous I/O requests and completions, persistent workers
and tasks, GPU objects, staging bytes, and committed and resident VRAM. Each
provider reports baseline, peak, final, acquired, and released values after at
least 10,000 iterations.

Available providers must balance their lifecycle totals and return to the
baseline. Retained resources fail with `test.002d.leak`; inconsistent peaks,
ordering, unsupported-provider measurements, and VRAM residency above committed
memory fail closed. A provider for a capability not yet implemented is retained
in canonical order with `available=false` and zero metrics—never silently
substituted. The JSON fixtures are synthetic contract data, not I/O or GPU
hardware certification.

The native policy is `testing.leak`; serialized acceptance uses
`scripts/check_leak_soak_evidence.py`. Run
`tests/misc_root_tests/seen_leak_soak_contract.sh`, setting
`SEEN_TEST_002D_FUZZ_SECONDS=60` for the required seed-1101 fuzz duration. The
compiling example is `compiler_seen/examples/test_leak_soak.seen`, and the
native 10,000-validation soak is
`tests/fixtures/soak/test_002d_leak_soak.seen`.

## Benchmark baseline and regression evidence

TEST-002C defines `seen-benchmark-evidence-v1`, a strict, canonical evidence
tuple for Colibri performance claims. Each record binds the hardware,
toolchain, source and baseline commits, exact command, read-back cgroup limits,
five warmups, thirty sorted nanosecond samples, their median, bandwidth, peak
RSS and VRAM, transfer volume, correctness, fallback count, backend maturity,
and whether real hardware executed. Unknown or duplicate fields fail closed.

Designated kernels have a hard five-percent median regression ceiling. They
must execute on hardware, may not use fallback, and cannot use `unsupported` or
`compile-only` evidence. CPU records cannot claim VRAM, transfers, or GPU
execution. The RTX 4090 JSON under `tests/fixtures/test-002c` is synthetic
contract data only; it is not hardware certification.

The native policy lives in `testing.benchmark`; the Python checker only
validates serialized acceptance evidence. Run
`tests/misc_root_tests/seen_benchmark_evidence_contract.sh`, setting
`SEEN_TEST_002C_FUZZ_SECONDS=60` for required seed-1101 fuzz evidence. The
compiling example is `compiler_seen/examples/test_benchmark_evidence.seen`.
Legacy `scripts/perf_gate.sh` suite baselines remain useful general regression
signals, but cannot substitute for this Colibri evidence tuple.

## Fuzz corpus minimization and replay

TEST-002B defines `seen-test-fuzz-corpus-v1`. A corpus manifest pins its seed,
target, maximum input size, required replay count, and canonical byte-ordered
entries. Each entry records its original and minimized sizes, lowercase SHA-256
content address, stable `test.*` failure code, and the failure code observed by
every replay. Unknown fields, duplicate identities or payloads, unstable
replays, non-canonical order, and oversized input fail closed.

Minimized payloads live in `cases/<sha256>.bin` beside the manifest. Validation
requires contained, non-symlink regular files whose length and digest match the
manifest. The minimizer uses deterministic chunk deletion and accepts a change
only while the caller's failure predicate continues to reproduce. Canonical
output is written with an fsync-and-rename transaction; cancellation and write
failures remove temporary output. Corpus parsing, ordering, bounds, and replay
policy are native Seen; the Python checker is acceptance tooling for serialized
fixtures and never supplies production policy.

The compiling example is
`compiler_seen/examples/test_fuzz_corpus.seen`. The acceptance command is
`tests/misc_root_tests/seen_fuzz_corpus_contract.sh`; set
`SEEN_TEST_002B_FUZZ_SECONDS=60` for the required seed-1101 fuzz duration.

## Sanitizer and coverage profiles

TEST-002A defines `seen-test-instrumentation-v1`. The current unreleased 0.23.0
compiler exposes `default`, `ci`, `coverage`, `sanitizer-undefined`, and
`sanitizer-thread` through `seen test <path> --profile <profile>`; published
0.22.11 does not include that command. Maintained acceptance harnesses include
`tests/misc_root_tests/seen_test_instrumentation_contract.sh` and
`tests/misc_root_tests/seen_test_runner_contract.sh`. Compiler-capable invocations
must use the verified containment described in [bootstrap.md](bootstrap.md).
Seen test
sources are compiled with the matching compiler instrumentation, execute under
the existing per-test timeout and the serialized `--no-fork` compiler path,
and keep reports and raw coverage profiles below
`.seen_cache/test/run`. Coverage mode also merges every raw profile with
`llvm-profdata` and emits a text report with `llvm-cov`; missing post-processing
tools are an infrastructure failure, never an implicit downgrade.
Coverage export/report and profile merge explicitly use one worker, including
the compiler/GPU execution acceptance, so post-processing stays within the same
aggregate task ceiling as compilation and execution.
On LLD targets, no-fork mode also pins the linker and ThinLTO backend to one
worker so the compilation remains valid inside the aggregate task cap.
Shell regressions receive the selected compiler, source root and package
client. Their stale caller-specific active-scope markers are cleared so each
self-wrapping harness re-enters and verifies its own binding. Kernel limits,
serial worker settings, project namespace and serializer proof remain inherited;
the shell test cannot substitute a different binding or skip verification.

Installed-layout regression workers first establish a fresh capped binding for
the copied compiler. The attested runner's `--consumer-context <project>` mode
verifies the original namespace, numeric limits and serializer before passing a
canonical nested project/cache directory to that compiler. It admits only
`check` and `compile`, requires contained canonical source/output paths, rejects
symlinks and control characters, and clears checkout source/client/data overrides.
It does not relocate attestation records or relax memory, task or timeout limits.
The payload gate hides checkout library/runtime sources, verifies a read-only
prefix, exercises cold/warm fast and ThinLTO execution with CPU-baseline auditing,
and tests real runtime-source compilation and blocked-cache failures. Compiler
stand-ins and stale installed-object fallbacks cannot satisfy those controls.

The incremental CLI regression uses a disposable project below the verified
artifact root. It exercises cold, warm and `--no-cache` compilation/execution,
requires actual warm source-cache hits and checks that uncached compilation
does not alter the warm cache. It never clears the checkout's shared cache.

The PGO CLI regression collects and merges profiles serially in its own artifact
directory. ThinLTO and full-LTO cycles must contain counters for the Seen entry
and imported helper functions, then execute the profile-use binary with identical
results. C-runtime-only profiles, missing profiles and unavailable merge tools
are failures, not successful skips.

The SIMD CLI tests declare `linux-x86_64` in discovery through their filename
markers. They verify native x86-64-v3 CPU/OS support before running AVX2/v3
executables, check scalar/deterministic executable baselines, and compare exact
Float64 math results across scalar, AVX2 and auto modes. Structured frontend and
LLVM reports must parse and must not claim hardware certification. The
deterministic success input is pure; a separate printing input must fail with
the typed nondeterministic-effect error. Unsupported backends must return an
error without producing an executable or claiming build success.
The separate Linux x86-64 toolchain-driver-alias regression executes the bounded
ELF linker alias and compiles/links/runs a C++ allocation witness. It protects
multicall-driver identity after resolving symlinks, not just version output.

The Linux LLVM failure-probe regression exercises real optimizer and object
emission failures in fast and ThinLTO compilation. The explicit attested-runner
option `--llvm-failure-probe opt|object` admits only the fixed two-module fixture,
uncached serial compilation, a fresh private output and the verified reserved
scope. Bounded tool wrappers retain their real executable identities and limits;
the probe can only return a failure at the selected phase, never create or repair
IR, objects or executables. Ordinary invocations clear ambient probe settings.
The regression requires successful native baseline execution and rejection of
wrong scopes, sources, output paths, stages and arguments.

The historical fix-regression suite retains its import, manifest, ABI, semantic,
package and recovery cases under the same attested execution contract. It uses
unique project-private workspaces and explicit per-output LLVM exports instead
of deleting shared caches or inspecting global temporary files. LLVM phase
failure cases delegate to the fixed failure-probe regression. Its separate C12
late-declaration negative requires LLVM verification followed by the expected
missing-symbol linker failure; that result is not successful native execution.

Allocator expected-abort source inputs live in `tests/compiler_regressions/`,
not the automatically executed `compiler_seen/tests/` root. The discovered
`seen_allocator_oom_exit.sh` harness compiles all three, requires exit134 and
their specific checked-allocation diagnostics, and also executes the positive
small-budget case. They are tested negative inputs, not ignored unit tests.

The compiler accepts AddressSanitizer and MemorySanitizer build flags, but they
are not general runnable `seen test` profiles under the normal finite VMEM
ceiling. The runner rejects those profile names instead of silently relaxing
limits. Reviewed ASan contracts, including `test_sanitizers.sh`, use the separate
generated-executable runner: its finite shadow allowance is read back while
kernel RAM, zero-swap, task and timeout containment remain enforced. That narrow
ASan admission does not qualify MemorySanitizer or ThreadSanitizer execution.
The CLI `sanitizer-thread` profile enters an attested capped binding, compiles
through the selected compiler runner, and uses the reviewed generated-executable
admission path rather than launching its binary directly. Missing scope,
refused admission and sanitizer startup failure remain failed tests with nonzero
exit status. Stock LLVM 20's requirement for unlimited `RLIMIT_AS` is not
admitted by the finite-address policy. Core and gpu-runtime-async CI provisioning
now explicitly builds the finite-address SDK from authenticated LLVM 20.1.2
sources; other jobs retain stock resources. The producer must pass real clean,
deliberate-race and insufficient-64-TiB controls before publishing its manifest.
The SDK-only prototype passed those controls; fresh integrated production/Seen
acceptance remains unverified until the required gates pass.

SDK selection requires both `SEEN_TSAN_SDK_MANIFEST` and the independently
published `SEEN_TSAN_SDK_MANIFEST_SHA256`. Internally consistent cache hashes
alone cannot authorize reuse: provisioning builds fresh and publishes a digest
outside the cached payload. Clang wrappers verify the selected inputs and consume
sealed runtime snapshots over the genuine stock resource tree in a private,
read-only namespace; cache replacement cannot change already sealed bytes.
This path rejects pre-terminator response files, explicit compiler configs and
`CCC_OVERRIDE_OPTIONS` rather than allowing hidden resource-directory overrides.
It preserves positional `--` and Clang++ driver selection. Bubblewrap represents
a signaled tool as shell status `128 + signal`, not raw waitpid signal identity.
No source/IR rewrite, ASLR change, unlimited address space, sanitizer suppression,
aggregate-cap increase or success fallback is admitted.

CORE-004E signal failures retain only allowlisted invocation metadata and bounded
source/compiler/program identities, plus a complete ELF up to 24 MiB or an
explicit omission. The collector rejects duplicate JSON keys and changed,
unsafe or mismatched files; complete upload files are published from staging
outside the upload directory. Discovery includes guarded standalone scopes and
private-`/tmp` rebuild compiler aliases verified against their project-backed
mount. Crash payloads share the existing 32 MiB / 128-file upload budget,
including the index reservation. This is diagnostic evidence, not a reproduction
or repair claim for the previously observed ThinLTO SIGILL.

Strict vendor-only CUDA/Vulkan LSan findings remain on unchanged driver
615.78.08; no suppressions are admitted. A passing component soak or SDK-only
TSan prototype does not close those findings or certify production hardware.

Acceptance evidence names compiler host code, GPU emitters, Seen modules, the
native runtime, and ledgered ABI shims separately. Each must be compiled and
software-executed. GPU emitter execution proves the compiler path only:
`hardware_executed` must remain false until a separate real-hardware gate
produces hardware evidence. Compile-only evidence cannot close TEST-002A.
The capped execution acceptance therefore builds and runs an instrumented
compiler, exercises the software GPU emitter, then runs the instrumented Seen
fixture before deriving the five-component evidence document.
The native validators borrow their input plans and contexts. A successful
`Result` borrows the original plan too: release the result container separately
from caller-owned component arrays, components, plans and contexts. Validation
errors own their marker/container storage and must be released by the caller.
The ownership example is `compiler_seen/examples/test_instrumentation.seen`;
it validates supplied records without claiming actual hardware execution.
The named native cases also require exact return to their initial live-allocation
baseline rather than merely printing a cleanup marker.
Its measured lifecycle soak checks 10,000 iterations of accepted and rejected
plans, cancellation and unsupported targets. Result containers, owned errors,
components and contexts are explicitly released; live usage must return to the
initial baseline, allocation-failure counts must stay unchanged, and recyclable
reserve must remain stable after the first iteration. The soak runs in fast,
release/ThinLTO, UBSan, ASan/LSan and source-coverage modes. Sanitizer shadow
admission retains the enclosing kernel memory, zero-swap and task ceilings.

## Deterministic test discovery

TEST-001A defines the `seen-test-discovery-v1` manifest consumed by the test
runner. Discovery is byte-ordered by canonical repository-relative path. It
rejects traversal, symlinks, duplicate or unordered entries, unsupported file
shapes, unknown fields, and caller-supplied categories that disagree with the
path policy.

The maintained roots and primary categories are:

| Root | Runnable shape | Primary category |
|---|---|---|
| `compiler_seen/tests/` | `.seen` | `unit` |
| `seen_std/tests/` | `.seen` | `unit` |
| `tests/fixtures/external_package/tests/` | `.seen` | `integration` |
| `tests/misc_root_tests/` | `.sh`, `_unit.py`, `_test.seen` | `integration` |

Within the three package test roots, `*_support.seen` files are imported helper
modules and `*_fixture.seen` files are programs requiring their runnable
harness's launch context, such as arguments or captured startup environment.
Neither is a standalone test or an ignored test. The native
runner and host migration/discovery oracles exclude these exact suffixes and
reject injected test descriptors for them. Names such as
`example_support_test.seen` remain runnable. Harness inputs retain their owning
tests, arguments and assertions; moving an input into this role does not waive
its execution requirement.

Names may add orthogonal `ignored`, `slow`, or `privileged` markers using a
matching directory or underscore-delimited filename segment. Platform markers
are `linux`, `linux_x86_64`, `linux_arm64`, `macos`, and `windows`; unmarked
tests use `all`. These markers describe selection policy and do not grant
privileges or silently skip an unsupported platform.

Legacy sources that cannot be linked as standalone programs carry the explicit
`ignored` marker. They remain discoverable and reported, but run only after an
operator supplies `--include-ignored`; the runner never keeps a hidden skip
list.

Native Seen exposes `TestPrimaryCategory` and `TestPlatformCategory`; ignored,
slow, and privileged remain independent flags so categories compose without
stringly typed combinations.

`scripts/discover_seen_tests.py --discover <absolute-repository-root>` is the
strict host oracle for the current compiler's canonical `seen test` execution
command. It emits canonical JSON to standard output. Validation is available
with `--validate <manifest>` and never repairs input.

## Canonical test runner

The current unreleased 0.23.0 compiler's canonical `seen test <path>` command
consumes that native inventory through the `seen-test-run-v1` policy. Published
0.22.11 has no `test` command. Selection is deterministic and supports a filter,
the five profiles described above, explicit ignored/slow/privileged opt-ins, one
bounded worker, and a timeout of at most one hour. Its contract executes Seen,
shell, and Python test shapes through a narrow process boundary while retaining
per-test logs below `.seen_cache/test/run/`.

Exit codes are stable: `0` means all selected tests passed, `1` means at least
one selected test failed, `2` is invalid CLI or selection input, `3` is an
infrastructure or report-write failure, and `130` is cancellation. Timeouts are
test failures and never silently retry. JSON and JUnit reports are written
transactionally with repeatable `--report json:<path>` and
`--report junit:<path>` options; paths must remain below `.seen_cache/test/` or
`.seen/` and may not traverse symlinks or parent segments.

The legacy `scripts/run_all_tests.sh` continues to record and validate the same
discovery inventory while migration to the canonical source runner proceeds.

## Package-owned test migration

TEST-001F defines `seen-test-migration-v1`, the fail-closed ownership map for
compiler, standard-library, and external-package tests. Each source has an
explicit package root, manifest, tests root, category, platform, and bounded
test count. The host oracle rejects missing or symlinked topology, unsafe paths,
unknown or duplicate fields, reordered sources, inconsistent totals, and input
above the configured limits; native Seen exposes the equivalent typed plan and
stable `test.001f.*` errors.

The package-owned migration contract includes the external-package fixture
alongside compiler and standard-library tests. Every migrated `.seen` test runs
from its own manifest directory, preserving package dependency resolution rather
than assuming the repository root. CPU-only discovery does not locate or link
accelerator SDKs. Platform execution evidence belongs to the individual
acceptance harnesses; a portable schema is not a native execution claim.

## Assertions and snapshots

TEST-001C provides native `seen-test-assertion-v1` assertion evaluation and
`seen-test-snapshot-v1` exact-text snapshots. Assertions return structured
results: a comparison failure is test data (`passed: false`), while invalid,
cancelled, or over-limit requests return stable `test.001c.*` errors through a
fallible API. Names are bounded portable identifiers and values are limited to
1 MiB before allocation or any filesystem side effect.

Snapshots compare bytes exactly; newline, spacing, and Unicode differences are
significant. The typed update policy is `ReadOnly`, `CreateMissing`, or
`Replace`. Updates are never implicit: comparison returns `writeRequired`, and
the caller may then transactionally write the returned content to
`.seen_snapshots/<name>.snap`. CPU-only use has no accelerator dependency. The
contract is implemented for Linux x86-64 and is platform-neutral source on
Windows, macOS, and other declared Seen targets; filesystem persistence remains
the caller's bounded platform adapter.

## Deterministic isolated fixtures

TEST-001D defines the native `seen-test-fixture-v1` plan. A fixture has a
portable name, explicit deterministic seed and target, a byte-ordered unique
file set, and a byte-ordered environment allowlist limited to `LANG`,
`LC_ALL`, `TZ`, and `SEEN_TEST_*`. Ambient home, path, locale, timezone,
randomness, network, and clock state are not silently captured.

Plans validate every bound before materialization and use the owned root
`.seen_cache/test/fixtures/<name>`. A pre-existing root, traversal, symlinked
base, duplicate/unordered path, unsupported target, or unapproved environment
key or secret-bearing environment name fails closed. The materialization oracle creates files exclusively below a
new physical root and returns an ownership token required for idempotence-safe
cleanup; tests cover success, failure, cancellation, limits, and zero retained
fixture roots. Linux x86-64 is verified now; Linux ARM64, macOS, and Windows
share the declared source contract and require their target runner adapter.

## Human, JSON, and JUnit reports

TEST-001E defines `seen-test-report-v1`, one native validated rendering path
for deterministic human text, canonical `seen-test-run-v1` JSON, and escaped
JUnit XML. Report counters, status/exit-code pairs, byte-ordered paths, suite
identity, cancellation, and output bounds are validated before rendering.
Human output is the canonical CLI display; JSON and JUnit are written only to
the safe transactional report paths documented above. XML declarations and
entities are rejected by the host oracle, and all formats end deterministically.

## Formatter protocol acceptance

`compiler_seen/tests/lsp_formatter.seen` parses serialized TextEdit JSON and
checks the UTF-16 range and replacement string. Equivalent JSON number
representations must not fail an assertion about decimal spelling, whitespace,
or object member order.

`tests/lsp_formatter_jsonrpc.py` drives a real server and applies returned edits
to documents across the supported languages, including surrogate pairs and
percent-encoded file URIs. These protocol checks validate consumer-visible
formatting, not serializer presentation.

