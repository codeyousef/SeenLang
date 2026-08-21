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

    def test_symlink_root_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "physical"
            root.mkdir()
            link = Path(temporary) / "link"
            link.symlink_to(root, target_is_directory=True)
            with self.assertRaises(DISCOVERY.TestDiscoveryError) as raised:
                DISCOVERY.discover(link)
            self.assertEqual("invalid", raised.exception.code)


if __name__ == "__main__":
    unittest.main()
