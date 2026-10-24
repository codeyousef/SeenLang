#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "discovery", ROOT / "scripts/discover_seen_tests.py")
assert SPEC is not None and SPEC.loader is not None
DISCOVERY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DISCOVERY)


def manifest() -> dict[str, object]:
    paths = [
        "compiler_seen/tests/alpha_test.seen",
        "compiler_seen/tests/ignored/beta_test.seen",
        "seen_std/tests/slow/omega_test.seen",
        "seen_std/tests/zeta_test.seen",
        "tests/misc_root_tests/seen_linux_privileged_probe.sh",
    ]
    return {"schema": DISCOVERY.SCHEMA,
            "tests": [DISCOVERY.descriptor(path) for path in paths]}


class TestDiscoveryTests(unittest.TestCase):
    def raw(self, value: object | None = None) -> bytes:
        return json.dumps(manifest() if value is None else value).encode()

    def code(self, code: str, raw: bytes, **kwargs: object) -> None:
        with self.assertRaises(DISCOVERY.TestDiscoveryError) as raised:
            DISCOVERY.validate(raw, **kwargs)
        self.assertEqual(code, raised.exception.code)

    def test_happy_categories(self) -> None:
        tests = DISCOVERY.validate(self.raw())["tests"]
        self.assertEqual(["unit", "unit", "unit", "unit", "integration"],
                         [entry["category"] for entry in tests])
        self.assertTrue(tests[1]["ignored"])
        self.assertTrue(tests[2]["slow"])
        self.assertEqual("linux", tests[4]["platform"])
        self.assertTrue(tests[4]["privileged"])

    def test_invalid_limit_cancel(self) -> None:
        value = manifest()
        value["tests"][0]["path"] = "../escape.seen"
        self.code("invalid", self.raw(value))
        self.code("limit", self.raw(), max_tests=1)
        self.code("cancelled", self.raw(), cancelled=True)

    def test_duplicate_order_and_fuzz(self) -> None:
        self.code("invalid", b'{"schema":1,"schema":2}')
        value = manifest()
        value["tests"].reverse()
        self.code("invalid", self.raw(value))
        DISCOVERY.fuzz(self.raw(), 0.02, 1101)

    def test_canonical_path_boundaries(self) -> None:
        for path in ("a-z_A.Z/09.seen", "x" * DISCOVERY.MAX_PATH_BYTES):
            self.assertTrue(DISCOVERY.canonical_path(path), path)
        for path in ("", "/absolute.seen", "trailing/", "a//b.seen", "a/../b.seen",
                     "a\\b.seen", "with space.seen", "line\n.seen", "tail.seen\n",
                     "null\0.seen", "café.seen", "百分.seen", "x" * (DISCOVERY.MAX_PATH_BYTES + 1),
                     None, b"bytes.seen", 1):
            with self.subTest(path=path):
                self.assertFalse(DISCOVERY.canonical_path(path))

    def test_symlink_root_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "physical"
            root.mkdir()
            link = Path(temporary) / "link"
            link.symlink_to(root, target_is_directory=True)
            with self.assertRaises(DISCOVERY.TestDiscoveryError) as raised:
                DISCOVERY.discover(link)
            self.assertEqual("invalid", raised.exception.code)

    def test_allocator_failure_inputs_have_runnable_owner(self) -> None:
        # Expected-abort payloads are not exit-zero unit tests. Their owning
        # harness must remain selected and execute each payload explicitly.
        paths = {entry["path"]: entry for entry in DISCOVERY.discover(ROOT)["tests"]}
        owner = "tests/misc_root_tests/seen_allocator_oom_exit.sh"
        self.assertIn(owner, paths)
        self.assertFalse(paths[owner]["ignored"])
        source = (ROOT / owner).read_text()
        for name in ("allocator_oom_exit.seen", "allocator_aligned_oom_exit.seen",
                     "allocator_pool_oom_exit.seen"):
            fixture = "tests/compiler_regressions/" + name
            self.assertTrue((ROOT / fixture).is_file())
            self.assertIn(fixture, source)
            self.assertNotIn(fixture, paths)
            self.assertFalse((ROOT / "compiler_seen/tests" / name).exists())
        self.assertIn("process.returncode != 134", source)
        self.assertIn('"Seen allocation failure" not in stderr', source)

    def test_helper_and_harness_input_roles_are_not_standalone_tests(self) -> None:
        for root in ("compiler_seen/tests", "seen_std/tests",
                     "tests/fixtures/external_package/tests"):
            for suffix in ("_support.seen", "_fixture.seen"):
                path = f"{root}/sample{suffix}"
                self.assertEqual("", DISCOVERY.expected_category(path))
                with self.assertRaises(DISCOVERY.TestDiscoveryError):
                    DISCOVERY.descriptor(path)
                injected = {"schema": DISCOVERY.SCHEMA, "tests": [{
                    "category": "unit" if root != "tests/fixtures/external_package/tests" else "integration",
                    "path": path, "ignored": False, "slow": False,
                    "privileged": False, "platform": "all"}]}
                self.code("invalid", json.dumps(injected).encode())
            self.assertTrue(DISCOVERY.expected_category(f"{root}/sample_support_test.seen"))
            self.assertTrue(DISCOVERY.expected_category(f"{root}/sample_fixture_test.seen"))

    def test_harness_driven_programs_keep_runnable_owners(self) -> None:
        paths = {entry["path"]: entry for entry in DISCOVERY.discover(ROOT)["tests"]}
        cases = (
            ("async/io_completion_stress_fixture.seen", "seen_async_reactor_contract.sh"),
            ("async/io_completion_stress_fixture.seen", "seen_async_stress_contract.sh"),
            ("json/large_object_release_fixture.seen", "seen_json_large_object_release.sh"),
            ("memory/uint8_pointer_deref_fixture.seen", "seen_open_issue_regressions.sh"),
            ("time_basic_fixture.seen", "seen_time_contract.sh"),
        )
        for fixture, owner_name in cases:
            fixture_path = "seen_std/tests/" + fixture
            owner = "tests/misc_root_tests/" + owner_name
            self.assertTrue((ROOT / fixture_path).is_file())
            self.assertNotIn(fixture_path, paths)
            self.assertIn(owner, paths)
            self.assertFalse(paths[owner]["ignored"])
            self.assertIn(fixture_path, (ROOT / owner).read_text())
        for consumer, helper in (
                ("async/dns_wire_adverse.seen", "dns_wire_corpus_support"),
                ("async/dns_policy_adverse.seen", "dns_fixture_support"),
                ("async/async-002g.seen", "stress_fixture_support")):
            path = "seen_std/tests/" + consumer
            self.assertIn(path, paths)
            self.assertFalse(paths[path]["ignored"])
            self.assertIn("import " + helper + ".", (ROOT / path).read_text())

    def test_startup_clock_owner_keeps_bounded_inputs_and_attestation(self) -> None:
        owner = (ROOT / "tests/misc_root_tests/seen_time_contract.sh").read_text()
        for token in ("SEEN_ATTESTED_COMPILER_RUNNER", "--verify-active",
                      "env -i", "SEEN_DETERMINISTIC=1", "SOURCE_DATE_EPOCH=42",
                      "SEEN_DETERMINISTIC_SEED=0", "SEEN_HASH_SEED=0",
                      "TZ=UTC", "LANG=C", "LC_ALL=C",
                      'SEEN_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:?}"',
                      "SEEN_JOBS=1", "SEEN_OPT_JOBS=1"):
            self.assertIn(token, owner)
        self.assertNotIn("--jobs 1", owner)
        launch_loop = owner.index("    for mode in fast release; do")
        for mode in ("fast", "release"):
            audit = f'bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/$name-{mode}"'
            self.assertLess(owner.index(audit), launch_loop)
        fixture = (ROOT / "seen_std/tests/time_basic_fixture.seen").read_text()
        self.assertNotIn('set("SEEN_DETERMINISTIC", "1")', fixture)
        self.assertIn('expect(not set("SOURCE_DATE_EPOCH", "43")', fixture)


if __name__ == "__main__":
    unittest.main()
