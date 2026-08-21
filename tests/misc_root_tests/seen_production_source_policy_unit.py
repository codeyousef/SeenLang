#!/usr/bin/env python3
"""Branch matrix for the CORE-003D production-source policy oracle."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_production_source_policy",
    ROOT / "scripts/check_production_source_policy.py")
assert SPEC is not None and SPEC.loader is not None
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


def source(path: str = "src/main.seen", digest: str = "a" * 64) -> dict[str, str]:
    return {
        "checkout_sha256": digest,
        "compiler_input_sha256": digest,
        "path": path,
    }


def document(**changes: object) -> bytes:
    value: dict[str, object] = {
        "platform": "linux-x86_64",
        "rewrite_requested": False,
        "schema": "seen-production-source-input-v1",
        "sources": [source()],
    }
    value.update(changes)
    return json.dumps(value).encode()


class ProductionSourcePolicyTests(unittest.TestCase):
    def assert_code(self, code: str, raw: bytes, **limits: int | bool) -> None:
        with self.assertRaises(POLICY.PolicyError) as raised:
            POLICY.parse_and_validate(raw, **limits)
        self.assertEqual(code, raised.exception.code)

    def test_happy_is_sorted_and_rewrite_is_false(self) -> None:
        raw = document(sources=[source("z.seen", "b" * 64), source("a.seen")])
        self.assertEqual(
            {
                "ordered_paths": ["a.seen", "z.seen"],
                "rewrite_allowed": False,
                "schema": "seen-production-source-policy-v1",
                "source_count": 2,
            },
            POLICY.parse_and_validate(raw),
        )

    def test_input_and_limit_bounds(self) -> None:
        self.assert_code("limit", document(), max_bytes=0)
        self.assert_code("limit", document(), max_bytes=POLICY.MAX_INPUT_BYTES + 1)
        self.assert_code("limit", document(), max_sources=0)
        self.assert_code("limit", document(), max_sources=POLICY.MAX_SOURCES + 1)
        self.assert_code("limit", document(), max_path_bytes=0)
        self.assert_code("limit", document(), max_path_bytes=POLICY.MAX_PATH_BYTES + 1)
        self.assert_code("limit", document(sources=[]))
        self.assert_code(
            "limit", document(sources=[source(f"m{index}.seen") for index in range(2)]),
            max_sources=1)
        self.assert_code("limit", document(), max_bytes=1)

    def test_cancel_and_platform(self) -> None:
        self.assert_code("cancelled", document(), cancelled=True)
        self.assert_code("platform", document(platform="plan9"))

    def test_schema_shape_duplicate_and_bom(self) -> None:
        self.assert_code("invalid", document(schema="other"))
        self.assert_code("invalid", document(extra=True))
        self.assert_code("invalid", b'{"schema":1,"schema":2}')
        self.assert_code("invalid", b"\xef\xbb\xbf" + document())
        self.assert_code("invalid", b"{")

    def test_rewrite_is_forbidden(self) -> None:
        self.assert_code("invalid", document(rewrite_requested=True))
        self.assert_code("invalid", document(rewrite_requested=0))

    def test_paths_are_canonical_unique_and_bounded(self) -> None:
        for path in ("", "/main.seen", "a\\b.seen", "a\x00b.seen",
                     "a/../b.seen", "a//b.seen"):
            self.assert_code(
                "invalid" if path else "limit", document(sources=[source(path)]))
        self.assert_code("limit", document(), max_path_bytes=4)
        self.assert_code(
            "invalid", document(sources=[source("a.seen"), source("a.seen", "b" * 64)]))
        self.assert_code("invalid", document(sources=[source(path=1)]))

    def test_digest_shape_and_identity(self) -> None:
        for digest in ("A" * 64, "a" * 63, "g" * 64, 1):
            value = source()
            value["checkout_sha256"] = digest
            self.assert_code("invalid", document(sources=[value]))
        value = source()
        value["compiler_input_sha256"] = "b" * 64
        self.assert_code("invalid", document(sources=[value]))

    def test_source_shape_and_types(self) -> None:
        self.assert_code("invalid", document(sources="bad"))
        self.assert_code("invalid", document(sources=[1]))
        self.assert_code("invalid", document(sources=[{"path": "a.seen"}]))
        self.assert_code("invalid", document(sources=[source() | {"extra": 1}]))

    def test_fuzz_is_bounded_and_deterministic(self) -> None:
        POLICY.fuzz(document(), 0.05, 1101)

    def test_cli_success_fuzz_cancel_and_io(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "policy.json"
            path.write_bytes(document())
            with contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(0, POLICY.main([str(path)]))
                self.assertEqual(
                    0, POLICY.main([str(path), "--fuzz-seconds", "0.002",
                                    "--seed", "1101"]))
                self.assertEqual(1, POLICY.main([str(path), "--fuzz-seconds", "301"]))
                self.assertEqual(1, POLICY.main([str(path), "--fuzz-seconds", "-1"]))
                self.assertEqual(1, POLICY.main([str(path), "--test-cancel-after-read"]))
                os.environ["SEEN_PRODUCTION_SOURCE_TEST_HOOKS"] = "1"
                try:
                    self.assertEqual(
                        130, POLICY.main([str(path), "--test-cancel-after-read"]))
                finally:
                    del os.environ["SEEN_PRODUCTION_SOURCE_TEST_HOOKS"]
                self.assertEqual(1, POLICY.main([str(path) + ".missing"]))


if __name__ == "__main__":
    unittest.main()
