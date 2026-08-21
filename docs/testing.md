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

Native Seen exposes `TestPrimaryCategory` and `TestPlatformCategory`; ignored,
slow, and privileged remain independent flags so categories compose without
stringly typed combinations.

`scripts/discover_seen_tests.py --discover <absolute-repository-root>` is the
strict host oracle used before TEST-001B ships the canonical `seen test`
execution command. It emits canonical JSON to standard output. Validation is
available with `--validate <manifest>` and never repairs input.

The legacy `scripts/run_all_tests.sh` records the discovered manifest below
the validated project artifact root before it starts its maintained test list.
TEST-001B will consume the same native contract for filtering, execution, exit
codes, timeouts, and reports.
