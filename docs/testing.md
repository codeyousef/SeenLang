# Testing

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
