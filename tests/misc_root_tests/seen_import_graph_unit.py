#!/usr/bin/env python3
"""Unit and branch coverage for the deterministic import-graph oracle."""

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
    "check_import_graph", ROOT / "scripts/check_import_graph.py"
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("could not load import-graph checker")
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
HAPPY_PATH = ROOT / "tests/fixtures/core-003a/happy/graph.json"
HAPPY = json.loads(HAPPY_PATH.read_bytes())


class ImportGraphTests(unittest.TestCase):
    def assert_code(self, document: object, code: str, **limits: int) -> None:
        with self.assertRaises(CHECKER.ContractError) as raised:
            graph = CHECKER.validate(
                document,
                limits.get("max_modules", CHECKER.MAX_MODULES),
                limits.get("max_edges", CHECKER.MAX_EDGES),
            )
            CHECKER.resolve(graph, limits.get("max_depth", CHECKER.MAX_DEPTH))
        self.assertEqual(raised.exception.code, code)

    def test_happy_is_canonical_across_edge_order(self) -> None:
        graph = CHECKER.validate(copy.deepcopy(HAPPY), 4096, 65_536)
        expected = CHECKER.resolve(graph, 4096)
        reversed_edges = copy.deepcopy(HAPPY)
        reversed_edges["edges"].reverse()
        self.assertEqual(
            CHECKER.resolve(CHECKER.validate(reversed_edges, 4096, 65_536), 4096),
            expected,
        )
        self.assertEqual(expected["order"], [0, 1, 3, 2])

    def test_exact_shapes_and_schema(self) -> None:
        self.assert_code({}, "invalid")
        unknown = copy.deepcopy(HAPPY)
        unknown["unknown"] = True
        self.assert_code(unknown, "invalid")
        bad_edge = copy.deepcopy(HAPPY)
        bad_edge["edges"][0]["unknown"] = True
        self.assert_code(bad_edge, "invalid")
        schema = copy.deepcopy(HAPPY)
        schema["schema"] = "other"
        self.assert_code(schema, "invalid")

    def test_module_and_edge_validation(self) -> None:
        for module in (
            "",
            "bad\x00path.seen",
            "/absolute.seen",
            "a\\b.seen",
            "a//b.seen",
            "a/../b.seen",
        ):
            changed = copy.deepcopy(HAPPY)
            changed["modules"][1] = module
            self.assert_code(
                changed,
                "limit" if module in {"", "bad\x00path.seen"} else "invalid",
            )
        for module in (None, "\ud800"):
            changed = copy.deepcopy(HAPPY)
            changed["modules"][1] = module
            self.assert_code(changed, "invalid")
        duplicate = copy.deepcopy(HAPPY)
        duplicate["modules"][2] = duplicate["modules"][1]
        self.assert_code(duplicate, "invalid")
        root = copy.deepcopy(HAPPY)
        root["root"] = root["modules"][1]
        self.assert_code(root, "invalid")
        edge = copy.deepcopy(HAPPY)
        edge["edges"][0]["to"] = "missing.seen"
        self.assert_code(edge, "invalid")
        duplicate_edge = copy.deepcopy(HAPPY)
        duplicate_edge["edges"].append(copy.deepcopy(duplicate_edge["edges"][0]))
        self.assert_code(duplicate_edge, "invalid")

    def test_platform_cycle_and_limits(self) -> None:
        platform = copy.deepcopy(HAPPY)
        platform["platform"] = "plan9"
        self.assert_code(platform, "platform")
        cycle = {
            "schema": "seen-import-graph-v1",
            "platform": "linux-x86_64",
            "root": "a.seen",
            "modules": ["a.seen", "b.seen"],
            "edges": [
                {"from": "a.seen", "to": "b.seen"},
                {"from": "b.seen", "to": "a.seen"},
            ],
        }
        self.assert_code(cycle, "cycle")
        self.assert_code(HAPPY, "limit", max_modules=1)
        self.assert_code(HAPPY, "limit", max_edges=1)
        self.assert_code(HAPPY, "limit", max_depth=1)
        self.assert_code(HAPPY, "limit", max_modules=0)

    def test_invalid_json_bytes_and_cancellation(self) -> None:
        raw = HAPPY_PATH.read_bytes()
        for invalid in (b"{", b"\xff", b"\xef\xbb\xbf{}", b'{"schema":1,"schema":2}'):
            with self.assertRaises(CHECKER.ContractError):
                CHECKER.parse_and_resolve(invalid, 1_048_576, 4096, 65_536, 4096)
        with self.assertRaises(CHECKER.ContractError) as raised:
            CHECKER.parse_and_resolve(raw, 1, 4096, 65_536, 4096)
        self.assertEqual(raised.exception.code, "limit")
        graph = CHECKER.validate(HAPPY, 4096, 65_536)
        with self.assertRaises(CHECKER.ContractError) as raised:
            CHECKER.resolve(graph, 4096, True)
        self.assertEqual(raised.exception.code, "cancelled")

    def test_seeded_fuzz(self) -> None:
        CHECKER.fuzz(HAPPY_PATH.read_bytes(), 0.02, 1101)

    def run_main(self, path: Path, *args: str) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            status = CHECKER.main([str(path), *args])
        return status, stdout.getvalue(), stderr.getvalue()

    def test_main_success_fuzz_errors_and_cancel_hook(self) -> None:
        status, output, error = self.run_main(
            HAPPY_PATH, "--fuzz-seconds", "0.01", "--seed", "1101"
        )
        self.assertEqual(status, 0)
        self.assertEqual(json.loads(output)["order"], [0, 1, 3, 2])
        self.assertIn("status=pass", error)
        self.assertEqual(self.run_main(HAPPY_PATH, "--fuzz-seconds", "-1")[0], 1)
        self.assertEqual(self.run_main(HAPPY_PATH.with_name("missing.json"))[0], 1)
        self.assertEqual(self.run_main(HAPPY_PATH, "--test-cancel-after-read")[0], 1)
        original = os.environ.get("SEEN_IMPORT_GRAPH_TEST_HOOKS")
        try:
            os.environ["SEEN_IMPORT_GRAPH_TEST_HOOKS"] = "1"
            status, output, error = self.run_main(
                ROOT / "tests/fixtures/core-003a/cancel/graph.json",
                "--test-cancel-after-read",
            )
        finally:
            if original is None:
                os.environ.pop("SEEN_IMPORT_GRAPH_TEST_HOOKS", None)
            else:
                os.environ["SEEN_IMPORT_GRAPH_TEST_HOOKS"] = original
        self.assertEqual(status, 130)
        self.assertEqual(output, "")
        self.assertIn("core.003a.cancelled", error)


if __name__ == "__main__":
    unittest.main()
