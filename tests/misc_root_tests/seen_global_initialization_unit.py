#!/usr/bin/env python3
"""Unit and branch coverage for the CORE-003B test oracle."""

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
    "check_global_initialization", ROOT / "scripts/check_global_initialization.py"
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("could not load global-initialization checker")
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
HAPPY_PATH = ROOT / "tests/fixtures/core-003b/happy/plan.json"
HAPPY = json.loads(HAPPY_PATH.read_bytes())


class GlobalInitializationTests(unittest.TestCase):
    def assert_code(self, document: object, code: str, **limits: int) -> None:
        with self.assertRaises(CHECKER.ContractError) as raised:
            value = CHECKER.validate(
                document,
                limits.get("max_modules", CHECKER.MAX_MODULES),
                limits.get("max_edges", CHECKER.MAX_EDGES),
            )
            CHECKER.plan(value, limits.get("max_depth", CHECKER.MAX_DEPTH))
        self.assertEqual(raised.exception.code, code)

    def test_dependency_first_order_is_stable(self) -> None:
        expected = CHECKER.plan(CHECKER.validate(HAPPY, 4096, 65_536), 4096)
        reversed_edges = copy.deepcopy(HAPPY)
        reversed_edges["edges"][0].reverse()
        actual = CHECKER.plan(CHECKER.validate(reversed_edges, 4096, 65_536), 4096)
        self.assertEqual(actual, expected)
        self.assertEqual(expected["order"], [3, 1, 2, 0])

    def test_exact_shape_schema_and_platform(self) -> None:
        self.assert_code({}, "invalid")
        unknown = copy.deepcopy(HAPPY)
        unknown["unknown"] = True
        self.assert_code(unknown, "invalid")
        schema = copy.deepcopy(HAPPY)
        schema["schema"] = "other"
        self.assert_code(schema, "invalid")
        platform = copy.deepcopy(HAPPY)
        platform["platform"] = "plan9"
        self.assert_code(platform, "platform")

    def test_module_validation(self) -> None:
        for module, code in (
            ("", "limit"), ("bad\x00path.seen", "limit"),
            ("/absolute.seen", "invalid"), ("a\\b.seen", "invalid"),
            ("a//b.seen", "invalid"), ("a/../b.seen", "invalid"),
            (None, "invalid"), ("\ud800", "invalid"),
        ):
            changed = copy.deepcopy(HAPPY)
            changed["modules"][1] = module
            self.assert_code(changed, code)
        duplicate = copy.deepcopy(HAPPY)
        duplicate["modules"][2] = duplicate["modules"][1]
        self.assert_code(duplicate, "invalid")

    def test_edge_validation_and_limits(self) -> None:
        wrong_shape = copy.deepcopy(HAPPY)
        wrong_shape["edges"].pop()
        self.assert_code(wrong_shape, "invalid")
        wrong_list = copy.deepcopy(HAPPY)
        wrong_list["edges"][0] = "1"
        self.assert_code(wrong_list, "invalid")
        for target in (True, "1", -1, 99):
            bad = copy.deepcopy(HAPPY)
            bad["edges"][0] = [target]
            self.assert_code(bad, "invalid")
        duplicate = copy.deepcopy(HAPPY)
        duplicate["edges"][0] = [1, 1]
        self.assert_code(duplicate, "invalid")
        self.assert_code(HAPPY, "limit", max_modules=1)
        self.assert_code(HAPPY, "limit", max_edges=1)
        self.assert_code(HAPPY, "limit", max_modules=0)
        self.assert_code(HAPPY, "limit", max_depth=1)

    def test_cycle_disconnected_and_cancel(self) -> None:
        cycle = {
            "schema": "seen-global-initialization-input-v1",
            "platform": "linux-x86_64",
            "modules": ["a.seen", "b.seen"],
            "edges": [[1], [0]],
        }
        self.assert_code(cycle, "invalid")
        disconnected = copy.deepcopy(HAPPY)
        disconnected["edges"] = [[], [], [], []]
        result = CHECKER.plan(CHECKER.validate(disconnected, 4096, 65_536), 4096)
        self.assertEqual(result["order"], [0, 1, 2, 3])
        value = CHECKER.validate(HAPPY, 4096, 65_536)
        with self.assertRaises(CHECKER.ContractError) as raised:
            CHECKER.plan(value, 4096, True)
        self.assertEqual(raised.exception.code, "cancelled")

    def test_invalid_bytes_bounds_and_fuzz(self) -> None:
        raw = HAPPY_PATH.read_bytes()
        for invalid in (b"{", b"\xff", b"\xef\xbb\xbf{}", b'{"schema":1,"schema":2}'):
            with self.assertRaises(CHECKER.ContractError):
                CHECKER.parse_and_plan(invalid, 1_048_576, 4096, 65_536, 4096)
        with self.assertRaises(CHECKER.ContractError) as raised:
            CHECKER.parse_and_plan(raw, 1, 4096, 65_536, 4096)
        self.assertEqual(raised.exception.code, "limit")
        CHECKER.fuzz(raw, 0.02, 1101)

    def run_main(self, path: Path, *args: str) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            status = CHECKER.main([str(path), *args])
        return status, stdout.getvalue(), stderr.getvalue()

    def test_main_success_errors_fuzz_and_cancel_hook(self) -> None:
        status, output, error = self.run_main(
            HAPPY_PATH, "--fuzz-seconds", "0.01", "--seed", "1101"
        )
        self.assertEqual(status, 0)
        self.assertEqual(json.loads(output)["order"], [3, 1, 2, 0])
        self.assertIn("status=pass", error)
        self.assertEqual(self.run_main(HAPPY_PATH, "--fuzz-seconds", "-1")[0], 1)
        self.assertEqual(self.run_main(HAPPY_PATH.with_name("missing.json"))[0], 1)
        self.assertEqual(self.run_main(HAPPY_PATH, "--test-cancel-after-read")[0], 1)
        old = os.environ.get("SEEN_GLOBAL_INIT_TEST_HOOKS")
        try:
            os.environ["SEEN_GLOBAL_INIT_TEST_HOOKS"] = "1"
            status, output, error = self.run_main(
                ROOT / "tests/fixtures/core-003b/cancel/plan.json",
                "--test-cancel-after-read",
            )
        finally:
            if old is None:
                os.environ.pop("SEEN_GLOBAL_INIT_TEST_HOOKS", None)
            else:
                os.environ["SEEN_GLOBAL_INIT_TEST_HOOKS"] = old
        self.assertEqual(status, 130)
        self.assertEqual(output, "")
        self.assertIn("core.003b.cancelled", error)


if __name__ == "__main__":
    unittest.main()
