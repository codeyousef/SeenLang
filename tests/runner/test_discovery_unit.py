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


if __name__ == "__main__":
    unittest.main()
