#!/usr/bin/env python3
import copy
import hashlib
import importlib.util
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release_artifacts", ROOT / "scripts/check_release_artifact_manifest.py")
checker = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(checker)
FIXTURE = ROOT / "tests/fixtures/core-004b/happy/manifest.json"


def manifest():
    return json.loads(FIXTURE.read_text())


class Tests(unittest.TestCase):
    def reject(self, value, code=None):
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(value)
        if code:
            self.assertEqual(raised.exception.code, code)

    def test_happy_primitives_and_canonical(self):
        value = manifest()
        self.assertEqual(checker.parse(FIXTURE.read_bytes())["schema"], checker.SCHEMA)
        self.assertEqual(checker.canonical(value), FIXTURE.read_bytes())
        self.assertTrue(checker.integer(1)); self.assertFalse(checker.integer(True))
        self.assertTrue(checker.digest("a" * 64)); self.assertFalse(checker.digest("A" * 64))
        self.assertTrue(checker.safe_name("seen-0.10.1")); self.assertFalse(checker.safe_name("../seen"))
        self.assertFalse(checker.safe_name("-seen")); self.assertFalse(checker.safe_name("."))
        self.assertTrue(checker.semver("0.11.0-rc.1")); self.assertFalse(checker.semver("v0.11.0"))
        self.assertFalse(checker.semver("0.11")); self.assertFalse(checker.semver("01.0.0"))
        self.assertEqual(checker.pairs([("a", 1)]), {"a": 1})
        with self.assertRaises(checker.ContractError): checker.pairs([("a", 1), ("a", 2)])

    def test_json_cancel_and_limits(self):
        for raw in (b"{", b"\xff", b'[]', b'{"schema":1,"schema":2}'):
            with self.assertRaises(checker.ContractError): checker.parse(raw)
        with self.assertRaises(checker.ContractError) as raised: checker.parse(FIXTURE.read_bytes(), cancelled=True)
        self.assertEqual(raised.exception.code, "cancelled")
        with self.assertRaises(checker.ContractError): checker.parse(FIXTURE.read_bytes(), max_bytes=1)
        with self.assertRaises(checker.ContractError): checker.parse(FIXTURE.read_bytes(), max_bytes=True)

    def test_manifest_rejections(self):
        good = manifest()
        mutations = []
        for key in checker.FIELDS:
            value = copy.deepcopy(good); value.pop(key); mutations.append(value)
        for key, bad in (("schema", "bad"), ("version", "01.0.0"), ("target", "windows-x86_64"),
                         ("source_commit", "0"), ("source_digest", "0")):
            value = copy.deepcopy(good); value[key] = bad; mutations.append(value)
        for value in mutations: self.reject(value)
        for key in checker.SIGNER_FIELDS:
            value = copy.deepcopy(good); value["signer"].pop(key); self.reject(value)
        value = copy.deepcopy(good); value["signer"]["mode"] = "none"; self.reject(value, "unsigned")
        value = copy.deepcopy(good); value["signer"]["identity"] = ""; self.reject(value, "unsigned")
        for key in checker.ARTIFACT_FIELDS:
            value = copy.deepcopy(good); value["artifacts"][0].pop(key); self.reject(value)
        value = copy.deepcopy(good); value["artifacts"] = value["artifacts"][:3]; self.reject(value, "limit")
        value = copy.deepcopy(good); value["artifacts"][1]["role"] = "stdlib"; self.reject(value)
        value = copy.deepcopy(good); value["artifacts"][1]["name"] = value["artifacts"][0]["name"]; self.reject(value)
        value = copy.deepcopy(good); value["artifacts"][0]["checksum"] = "other"; self.reject(value, "unsigned")
        value = copy.deepcopy(good); value["artifacts"][0]["bytes"] = checker.MAX_ARTIFACT_BYTES + 1; self.reject(value, "limit")

    def make_artifacts(self, root):
        artifacts = []
        for role in checker.ROLES:
            path = root / role
            path.write_bytes((role + "\n").encode())
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            Path(str(path) + ".sha256").write_text(digest + "\n")
            Path(str(path) + ".bundle").write_text("signed:" + role + "\n")
            artifacts.append((role, path))
        return artifacts

    def test_build_verify_mismatch_and_safety(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); artifacts = self.make_artifacts(root)
            value = checker.build("0.11.0", "linux-x86_64", "1" * 40, "2" * 64,
                "key", "test-key", "local-test", artifacts)
            checker.verify_files(value, root)
            artifacts[0][1].write_bytes(b"changed")
            with self.assertRaises(checker.ContractError) as raised: checker.verify_files(value, root)
            self.assertEqual(raised.exception.code, "mismatch")
            artifacts[0][1].unlink(); artifacts[0][1].symlink_to(artifacts[1][1])
            with self.assertRaises(checker.ContractError): checker.safe_file(artifacts[0][1])

    def test_atomic_cancel_fuzz_and_main(self):
        cases, rejected = checker.fuzz(FIXTURE.read_bytes(), .002, 1101)
        self.assertGreater(cases, 0); self.assertGreaterEqual(rejected, 0)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); output = root / "manifest.json"
            with self.assertRaises(checker.ContractError) as raised:
                checker.atomic_write(output, b"value", cancelled=True)
            self.assertEqual(raised.exception.code, "cancelled")
            self.assertFalse(output.exists()); self.assertEqual(list(root.glob(".*.tmp.*")), [])
            original_stdout, original_stderr = sys.stdout, sys.stderr
            class Output:
                def __init__(self): self.buffer = io.BytesIO()
            try:
                sys.stdout, sys.stderr = Output(), io.StringIO()
                self.assertEqual(checker.main(["--manifest", str(FIXTURE)]), 0)
                self.assertEqual(checker.main(["--manifest", str(root / "missing")]), 1)
                self.assertEqual(checker.main(["--manifest", str(FIXTURE), "--fuzz-seconds", "301"]), 1)
            finally:
                sys.stdout, sys.stderr = original_stdout, original_stderr


if __name__ == "__main__": unittest.main()
