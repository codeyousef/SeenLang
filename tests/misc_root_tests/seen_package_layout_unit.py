#!/usr/bin/env python3
"""Unit and branch coverage for the package-layout v1 test oracle."""

from __future__ import annotations

import copy
import importlib.util
import io
import json
import os
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_package_layout", ROOT / "scripts/check_package_layout.py"
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("could not load package-layout checker")
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
HAPPY = json.loads(
    (ROOT / "tests/fixtures/pkg-layout-001/happy/layout.json").read_bytes()
)


class PackageLayoutTests(unittest.TestCase):
    def assert_code(self, value: object, code: str) -> None:
        with self.assertRaises(CHECKER.ContractError) as raised:
            CHECKER.validate(value)
        self.assertEqual(raised.exception.code, code)

    def test_happy_and_canonical_bytes(self) -> None:
        self.assertEqual(CHECKER.validate(copy.deepcopy(HAPPY)), HAPPY)
        raw = (json.dumps(HAPPY, indent=2, sort_keys=True) + "\n").encode()
        self.assertEqual(CHECKER.parse_and_validate(raw, 65_536), HAPPY)

    def test_top_level_shape(self) -> None:
        self.assert_code({}, "invalid")
        unknown = copy.deepcopy(HAPPY)
        unknown["unknown"] = True
        self.assert_code(unknown, "invalid")

    def test_each_canonical_path(self) -> None:
        for field in CHECKER.EXPECTED_PATHS:
            changed = copy.deepcopy(HAPPY)
            changed[field] = "other"
            self.assert_code(changed, "invalid")

    def test_platform_shape_and_values(self) -> None:
        changed = copy.deepcopy(HAPPY)
        changed["platforms"] = {}
        self.assert_code(changed, "invalid")
        changed = copy.deepcopy(HAPPY)
        changed["platforms"]["windows"] = "required"
        self.assert_code(changed, "platform")

    def test_duplicate_and_invalid_json(self) -> None:
        duplicate = b'{"schema":"seen-package-layout-v1","schema":"other"}'
        with self.assertRaises(CHECKER.ContractError):
            CHECKER.parse_and_validate(duplicate, 65_536)
        for raw in (b"{", b"\xff", b"\xef\xbb\xbf{}"):
            with self.assertRaises(CHECKER.ContractError) as raised:
                CHECKER.parse_and_validate(raw, 65_536)
            self.assertEqual(raised.exception.code, "invalid")

    def test_limits(self) -> None:
        raw = json.dumps(HAPPY).encode()
        for maximum in (0, 65_537, 1):
            with self.assertRaises(CHECKER.ContractError) as raised:
                CHECKER.parse_and_validate(raw, maximum)
            self.assertEqual(raised.exception.code, "limit")

    def test_seeded_fuzz(self) -> None:
        raw = json.dumps(HAPPY).encode()
        CHECKER.fuzz(raw, 0.01, 1101, 65_536)

    def run_main(self, path: Path, *args: str) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            status = CHECKER.main([str(path), *args])
        return status, stdout.getvalue(), stderr.getvalue()

    def test_main_success_and_fuzz(self) -> None:
        path = ROOT / "tests/fixtures/pkg-layout-001/happy/layout.json"
        status, output, error = self.run_main(
            path, "--fuzz-seconds", "0.01", "--seed", "1101"
        )
        self.assertEqual(status, 0)
        self.assertEqual(json.loads(output), HAPPY)
        self.assertIn("status=pass", error)

    def test_main_bounds_and_missing_file(self) -> None:
        path = ROOT / "tests/fixtures/pkg-layout-001/happy/layout.json"
        self.assertEqual(self.run_main(path, "--fuzz-seconds", "-1")[0], 1)
        self.assertEqual(self.run_main(path, "--max-bytes", "1")[0], 1)
        self.assertEqual(self.run_main(path.with_name("missing.json"))[0], 1)

    def test_main_cancellation_hook_is_guarded(self) -> None:
        path = ROOT / "tests/fixtures/pkg-layout-001/cancel/layout.json"
        self.assertEqual(self.run_main(path, "--test-cancel-after-read")[0], 1)
        original = os.environ.get("SEEN_PKG_LAYOUT_TEST_HOOKS")
        try:
            os.environ["SEEN_PKG_LAYOUT_TEST_HOOKS"] = "1"
            status, output, error = self.run_main(
                path, "--test-cancel-after-read"
            )
        finally:
            if original is None:
                os.environ.pop("SEEN_PKG_LAYOUT_TEST_HOOKS", None)
            else:
                os.environ["SEEN_PKG_LAYOUT_TEST_HOOKS"] = original
        self.assertEqual(status, 130)
        self.assertEqual(output, "")
        self.assertIn("pkg.layout.001.cancelled", error)

    def test_main_invalid_fixture(self) -> None:
        path = ROOT / "tests/fixtures/pkg-layout-001/invalid/layout.json"
        status, _, error = self.run_main(path)
        self.assertEqual(status, 1)
        self.assertIn("pkg.layout.001.invalid", error)


if __name__ == "__main__":
    unittest.main()
