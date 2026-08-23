#!/usr/bin/env python3
import argparse
import copy
import importlib.util
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("gate0", ROOT / "scripts/check_gate0_certification.py")
checker = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(checker)
FIXTURE = ROOT / "tests/fixtures/p0-gate0-001/happy/evidence.json"


def evidence():
    return json.loads(FIXTURE.read_text())


class Tests(unittest.TestCase):
    def reject(self, value, code=None):
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(json.dumps(value).encode())
        if code:
            self.assertEqual(raised.exception.code, code)

    def test_happy_primitives_and_canonical(self):
        good = evidence()
        self.assertEqual(checker.validate(FIXTURE.read_bytes())["schema"], checker.SCHEMA)
        self.assertEqual(checker.canonical(good), FIXTURE.read_bytes())
        self.assertTrue(checker.integer(1)); self.assertFalse(checker.integer(True))
        self.assertTrue(checker.digest("a" * 64, 64)); self.assertFalse(checker.digest("A" * 64, 64))
        self.assertTrue(checker.safe_path("bootstrap/stage1_frozen"))
        for path in ("", "/absolute", "-option", "a\\b", "a//b", "a/../b", "a/./b"):
            self.assertFalse(checker.safe_path(path))
        self.assertEqual(checker.pairs([("a", 1)]), {"a": 1})
        with self.assertRaises(checker.ContractError): checker.pairs([("a", 1), ("a", 2)])

    def test_parse_cancel_and_limits(self):
        for raw in (b"{", b"\xff", b"[]", b'{"schema":1,"schema":2}'):
            with self.assertRaises(checker.ContractError): checker.validate(raw)
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(FIXTURE.read_bytes(), cancelled=True)
        self.assertEqual(raised.exception.code, "cancelled")
        for maximum in (0, True, checker.MAX_BYTES + 1):
            with self.assertRaises(checker.ContractError): checker.validate(FIXTURE.read_bytes(), maximum)
        with self.assertRaises(checker.ContractError): checker.validate(FIXTURE.read_bytes(), 1)

    def test_identity_policy_and_target_rejections(self):
        good = evidence()
        for key in checker.FIELDS:
            value = copy.deepcopy(good); value.pop(key); self.reject(value)
        for key, bad, code in (
            ("target", "windows", "platform"), ("schema", "bad", "invalid"),
            ("source_commit", "0", "invalid"), ("clean_checkout", False, "unverified"),
            ("active_ci", False, "unverified"), ("repair_allowed", True, "invalid"),
            ("disabled_workflow_count", 1, "invalid")):
            value = copy.deepcopy(good); value[key] = bad; self.reject(value, code)

    def test_limit_and_compiler_rejections(self):
        good = evidence()
        for key in checker.LIMIT_FIELDS:
            value = copy.deepcopy(good); value["limits"].pop(key); self.reject(value)
        for key, bad, code in (
            ("memory_max_bytes", 0, "limit"), ("memory_swap_max_bytes", 1, "limit"),
            ("pids_max", 25, "limit"), ("jobs", 2, "limit"),
            ("opt_jobs", 0, "limit"), ("timeout_seconds", 3601, "limit")):
            value = copy.deepcopy(good); value["limits"][key] = bad; self.reject(value, code)
        value = copy.deepcopy(good); value["limits"]["jobs"] = True; self.reject(value, "invalid")
        for key in checker.COMPILER_FIELDS:
            value = copy.deepcopy(good); value["compiler"].pop(key); self.reject(value)
        for key, bad in (("pinned", False), ("path", "../seen"), ("sha256", "0"),
                         ("compatibility_sha256", "0")):
            value = copy.deepcopy(good); value["compiler"][key] = bad; self.reject(value, "unverified")

    def test_step_and_platform_rejections(self):
        good = evidence()
        value = copy.deepcopy(good); value["steps"].pop(); self.reject(value, "limit")
        for key in checker.STEP_FIELDS:
            value = copy.deepcopy(good); value["steps"][0].pop(key); self.reject(value)
        for key, bad in (("name", "test"), ("status", "skipped"), ("artifact_sha256", "0")):
            value = copy.deepcopy(good); value["steps"][0][key] = bad; self.reject(value, "unverified")
        value = copy.deepcopy(good); value["platforms"].pop(); self.reject(value, "platform")
        for key in checker.PLATFORM_FIELDS:
            value = copy.deepcopy(good); value["platforms"][0].pop(key); self.reject(value)
        value = copy.deepcopy(good); value["platforms"][0]["support"] = "static-policy"; self.reject(value, "platform")

    def args(self, output):
        return argparse.Namespace(output=output, source_commit="1" * 40,
            compiler_sha256="a" * 64, compatibility_sha256="b" * 64,
            build_sha256="c" * 64, test_sha256="d" * 64,
            fuzz_sha256="e" * 64, package_sha256="f" * 64,
            memory_max_bytes=1024, pids_max=24, timeout_seconds=1800)

    def test_build_atomic_safe_read_fuzz_and_main(self):
        cases, rejected = checker.fuzz(FIXTURE.read_bytes(), .002, 1101)
        self.assertGreater(cases, 0); self.assertGreaterEqual(rejected, 0)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); output = root / "evidence.json"
            value = checker.build(self.args(output)); self.assertEqual(value["steps"][3]["name"], "package")
            with self.assertRaises(checker.ContractError): checker.atomic_write(output, b"x", True)
            self.assertFalse(output.exists()); self.assertEqual(list(root.glob(".*.tmp.*")), [])
            checker.atomic_write(output, checker.canonical(value)); self.assertEqual(checker.safe_read(output, checker.MAX_BYTES), output.read_bytes())
            link = root / "link"; link.symlink_to(output)
            with self.assertRaises(checker.ContractError): checker.safe_read(link, checker.MAX_BYTES)
            original_stdout, original_stderr = sys.stdout, sys.stderr
            class Output:
                def __init__(self): self.buffer = io.BytesIO()
            try:
                sys.stdout, sys.stderr = Output(), io.StringIO()
                self.assertEqual(checker.main(["--evidence", str(FIXTURE)]), 0)
                self.assertEqual(checker.main(["--evidence", str(root / "missing")]), 1)
                self.assertEqual(checker.main(["--evidence", str(FIXTURE), "--fuzz-seconds", "301"]), 1)
                self.assertEqual(checker.main([]), 1)
            finally:
                sys.stdout, sys.stderr = original_stdout, original_stderr


if __name__ == "__main__": unittest.main()
