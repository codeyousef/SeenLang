#!/usr/bin/env python3
"""Branch matrix for the CORE-003C production-IR policy oracle."""

from __future__ import annotations

import importlib.util
import contextlib
import io
import json
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_production_ir_policy", ROOT / "scripts/check_production_ir_policy.py")
assert SPEC is not None and SPEC.loader is not None
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


def artifact(path: str = "modules/main.ll", digest: str = "a" * 64) -> dict[str, str]:
    return {
        "emitted_sha256": digest,
        "optimizer_input_sha256": digest,
        "path": path,
    }


def document(**changes: object) -> bytes:
    value: dict[str, object] = {
        "artifacts": [artifact()],
        "platform": "linux-x86_64",
        "repair_requested": False,
        "schema": "seen-production-ir-input-v1",
    }
    value.update(changes)
    return json.dumps(value).encode()


class ProductionIrPolicyTests(unittest.TestCase):
    def assert_code(self, code: str, raw: bytes, **limits: int | bool) -> None:
        with self.assertRaises(POLICY.PolicyError) as raised:
            POLICY.parse_and_validate(raw, **limits)
        self.assertEqual(code, raised.exception.code)

    def test_happy_is_sorted_and_repair_is_false(self) -> None:
        raw = document(artifacts=[artifact("z.ll", "b" * 64), artifact("a.ll")])
        self.assertEqual(
            {
                "artifact_count": 2,
                "ordered_paths": ["a.ll", "z.ll"],
                "repair_allowed": False,
                "schema": "seen-production-ir-policy-v1",
            },
            POLICY.parse_and_validate(raw),
        )

    def test_input_and_limit_bounds(self) -> None:
        self.assert_code("limit", document(), max_bytes=0)
        self.assert_code("limit", document(), max_bytes=POLICY.MAX_INPUT_BYTES + 1)
        self.assert_code("limit", document(), max_artifacts=0)
        self.assert_code("limit", document(), max_artifacts=POLICY.MAX_ARTIFACTS + 1)
        self.assert_code("limit", document(), max_path_bytes=0)
        self.assert_code("limit", document(), max_path_bytes=POLICY.MAX_PATH_BYTES + 1)
        self.assert_code("limit", document(artifacts=[]))
        self.assert_code(
            "limit", document(artifacts=[artifact(f"m{index}.ll") for index in range(2)]),
            max_artifacts=1)
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

    def test_repair_is_forbidden(self) -> None:
        self.assert_code("invalid", document(repair_requested=True))
        self.assert_code("invalid", document(repair_requested=0))

    def test_paths_are_canonical_unique_and_bounded(self) -> None:
        for path in ("", "/main.ll", "a\\b.ll", "a\x00b.ll", "a/../b.ll", "a//b.ll"):
            self.assert_code("invalid" if path else "limit", document(artifacts=[artifact(path)]))
        self.assert_code("limit", document(), max_path_bytes=4)
        self.assert_code(
            "invalid", document(artifacts=[artifact("a.ll"), artifact("a.ll", "b" * 64)]))
        self.assert_code("invalid", document(artifacts=[artifact(path=1)]))

    def test_digest_shape_and_identity(self) -> None:
        for digest in ("A" * 64, "a" * 63, "g" * 64, 1):
            value = artifact()
            value["emitted_sha256"] = digest
            self.assert_code("invalid", document(artifacts=[value]))
        value = artifact()
        value["optimizer_input_sha256"] = "b" * 64
        self.assert_code("invalid", document(artifacts=[value]))

    def test_artifact_shape_and_types(self) -> None:
        self.assert_code("invalid", document(artifacts="bad"))
        self.assert_code("invalid", document(artifacts=[1]))
        self.assert_code("invalid", document(artifacts=[{"path": "a.ll"}]))
        self.assert_code("invalid", document(artifacts=[artifact() | {"extra": 1}]))

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
                    0, POLICY.main([str(path), "--fuzz-seconds", "0.002", "--seed", "1101"]))
                self.assertEqual(
                    1, POLICY.main([str(path), "--fuzz-seconds", "301"]))
                self.assertEqual(
                    1, POLICY.main([str(path), "--fuzz-seconds", "-1"]))
                self.assertEqual(
                    1, POLICY.main([str(path), "--test-cancel-after-read"]))
                os.environ["SEEN_PRODUCTION_IR_TEST_HOOKS"] = "1"
                try:
                    self.assertEqual(
                        130, POLICY.main([str(path), "--test-cancel-after-read"]))
                finally:
                    del os.environ["SEEN_PRODUCTION_IR_TEST_HOOKS"]
                self.assertEqual(1, POLICY.main([str(path) + ".missing"]))


if __name__ == "__main__":
    unittest.main()
