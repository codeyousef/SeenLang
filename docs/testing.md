# Testing

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

TEST-002A defines `seen-test-instrumentation-v1`. In addition to `default` and
`ci`, `seen test --profile` accepts `coverage`, `sanitizer-undefined`, and
`sanitizer-thread`. Seen test
sources are compiled with the matching compiler instrumentation, execute under
the existing per-test timeout and the serialized `--no-fork` compiler path,
and keep reports and raw coverage profiles below
`.seen_cache/test/run`. Coverage mode also merges every raw profile with
`llvm-profdata` and emits a text report with `llvm-cov`; missing post-processing
tools are an infrastructure failure, never an implicit downgrade.
On LLD targets, no-fork mode also pins the linker and ThinLTO backend to one
worker so the compilation remains valid inside the aggregate task cap.

The compiler still accepts AddressSanitizer and MemorySanitizer build flags as
compile-only capabilities. They are deliberately not runnable `seen test`
profiles under the mandatory finite VMEM ceiling because their multi-terabyte
shadow-address reservations cannot coexist with that containment. The runner
rejects those profile names instead of weakening limits or claiming execution.

Acceptance evidence names compiler host code, GPU emitters, Seen modules, the
native runtime, and ledgered ABI shims separately. Each must be compiled and
software-executed. GPU emitter execution proves the compiler path only:
`hardware_executed` must remain false until a separate real-hardware gate
produces hardware evidence. Compile-only evidence cannot close TEST-002A.
The capped execution acceptance therefore builds and runs an instrumented
compiler, exercises the software GPU emitter, then runs the instrumented Seen
fixture before deriving the five-component evidence document.

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
strict host oracle used before TEST-001B ships the canonical `seen test`
execution command. It emits canonical JSON to standard output. Validation is
available with `--validate <manifest>` and never repairs input.

## Canonical test runner

`seen test <project>` consumes that native inventory through the
`seen-test-run-v1` policy. Selection is deterministic and supports `--filter`,
the `default` and `ci` profiles, explicit ignored/slow/privileged opt-ins, one
bounded worker, and a timeout of at most one hour. The runner executes Seen,
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
discovery inventory while migration to the shipped runner proceeds.

## Package-owned test migration

TEST-001F defines `seen-test-migration-v1`, the fail-closed ownership map for
compiler, standard-library, and external-package tests. Each source has an
explicit package root, manifest, tests root, category, platform, and bounded
test count. The host oracle rejects missing or symlinked topology, unsafe paths,
unknown or duplicate fields, reordered sources, inconsistent totals, and input
above the configured limits; native Seen exposes the equivalent typed plan and
stable `test.001f.*` errors.

`seen test` discovers the external-package fixture alongside compiler and
standard-library tests, then executes every migrated `.seen` test from its own
manifest directory. This preserves package dependency resolution and removes
the former implicit assumption that all tests run from the repository root.
CPU-only discovery does not locate or link accelerator SDKs. Linux x86-64 is
the verified execution platform; the package-neutral schema remains portable.

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
