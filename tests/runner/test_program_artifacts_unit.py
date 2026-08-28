#!/usr/bin/env python3

import copy
import hashlib
import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/fixtures/core-004f"
POSITIVE_CASES = (
    "two-root-default", "cold-warm-no-cache", "release-full-lto",
    "release-thin-lto", "object-manifest", "package-graph",
    "installed-compiler", "path-remap",
)


def load_checker():
    spec = importlib.util.spec_from_file_location(
        "program_artifact_checker", ROOT / "scripts/check_program_artifacts.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load_checker()


def evidence(case="two-root-default"):
    return json.loads(
        (FIXTURES / case / "evidence.json").read_text(encoding="utf-8")
    )


def sha256(payload):
    return hashlib.sha256(payload).hexdigest()


def reseal(value):
    value["build_input"]["input_digest"] = checker.build_input_digest(
        value["build_input"]
    )
    input_digest = value["build_input"]["input_digest"]
    key = checker.cache_key(value["case"], input_digest)
    for result in value["results"]:
        result["build_input_digest"] = input_digest
        result["cache"]["input_digest"] = input_digest
        result["cache"]["key"] = key
        result["cache"]["artifact_set_sha256"] = checker.artifact_set_digest(
            result["artifacts"]
        )


class ProgramArtifactTests(unittest.TestCase):
    def reject(self, value, code):
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(json.dumps(value).encode("utf-8"))
        self.assertEqual(raised.exception.code, code)

    def test_all_named_positive_fixtures_are_canonical(self):
        for case in POSITIVE_CASES:
            with self.subTest(case=case):
                raw = (FIXTURES / case / "evidence.json").read_bytes()
                value = checker.validate(raw)
                self.assertEqual(checker.canonical(value), raw)
                self.assertEqual(value["case"], case)

    def test_schema_and_complete_input_identity_are_strict(self):
        schema = json.loads(
            (ROOT / "schemas/program-artifacts.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertFalse(schema["additionalProperties"])
        self.assertEqual(schema["properties"]["schema"]["const"], checker.SCHEMA)
        self.assertEqual(schema["properties"]["results"]["minItems"], 6)
        value = evidence()
        for field in sorted(checker.INPUT_FIELDS - {"input_digest"}):
            changed = copy.deepcopy(value)
            changed["build_input"].pop(field)
            self.reject(changed, "invalid")
        changed = copy.deepcopy(value)
        changed["build_input"]["features"] = ["z", "a"]
        reseal(changed)
        self.reject(changed, "invalid")
        changed = copy.deepcopy(value)
        changed["build_input"]["environment"].reverse()
        reseal(changed)
        self.reject(changed, "invalid")

    def test_named_negative_fixtures(self):
        expected = {
            "stale-cache": "stale_cache",
            "input-change": "input_change",
            "unsupported-combination": "unsupported_combination",
        }
        for name, code in expected.items():
            with self.subTest(name=name):
                raw = (FIXTURES / name / "evidence.json").read_bytes()
                with self.assertRaises(checker.ContractError) as raised:
                    checker.validate(raw)
                self.assertEqual(raised.exception.code, code)

    def test_cache_root_and_raw_parity_fail_closed(self):
        value = evidence()
        value["results"][3]["root_identity"] = value["results"][0][
            "root_identity"
        ]
        self.reject(value, "root_identity")
        value = evidence()
        value["results"][4]["artifacts"][0]["sha256"] = "f" * 64
        value["results"][4]["cache"]["artifact_set_sha256"] = (
            checker.artifact_set_digest(value["results"][4]["artifacts"])
        )
        self.reject(value, "mismatch")
        value = evidence()
        value["results"][1]["cache"]["state"] = "miss"
        self.reject(value, "stale_cache")

    def test_duplicate_malformed_limits_and_cancellation(self):
        duplicate = b'{"schema":"a","schema":"b"}'
        for raw in (duplicate, b"{", b"\xff", b"[]"):
            with self.assertRaises(checker.ContractError):
                checker.validate(raw)
        raw = (FIXTURES / "two-root-default/evidence.json").read_bytes()
        for maximum in (0, True, len(raw) - 1, checker.MAX_EVIDENCE_BYTES + 1):
            with self.assertRaises(checker.ContractError):
                checker.validate(raw, maximum)
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(raw, cancelled=True)
        self.assertEqual(raised.exception.code, "cancelled")

    def prepare_file_evidence(self, directory, case="two-root-default"):
        value = evidence(case)
        roots = {}
        executable_payload = b"deterministic executable\n"
        object_payload = b"deterministic object\n"
        manifest_payload = b"seen_module_0.o\tsrc/main.seen\n"
        for result in value["results"]:
            root = Path(directory) / result["id"]
            root.mkdir()
            roots[result["id"]] = root
            artifacts = []
            if case == "object-manifest":
                pins = (
                    ("seen_module_0.o", "object", object_payload, False),
                    ("objects.tsv", "object-manifest", manifest_payload, False),
                )
            else:
                pins = (("program", "executable", executable_payload, True),)
            for name, role, payload, executable in pins:
                path = root / name
                path.write_bytes(payload)
                if executable:
                    path.chmod(0o755)
                artifacts.append({"bytes": len(payload), "executable": executable,
                    "name": name, "role": role, "sha256": sha256(payload)})
            result["artifacts"] = artifacts
            result["cache"]["artifact_set_sha256"] = (
                checker.artifact_set_digest(artifacts)
            )
        return value, roots

    def test_file_pins_permissions_and_remapped_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            value, roots = self.prepare_file_evidence(directory, "object-manifest")
            checker.validate_value(value)
            checker.verify_files(value, roots)
            manifest = roots["root-b-warm"] / "objects.tsv"
            manifest.write_text(
                "seen_module_0.o\t/physical/root/src/main.seen\n",
                encoding="ascii",
            )
            with self.assertRaises(checker.ContractError) as raised:
                checker.verify_files(value, roots)
            self.assertIn(raised.exception.code, ("mismatch", "path_remap"))

    def test_symlinks_and_non_executable_outputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            value, roots = self.prepare_file_evidence(directory)
            checker.verify_files(value, roots)
            program = roots["root-a-warm"] / "program"
            program.chmod(0o644)
            with self.assertRaises(checker.ContractError) as raised:
                checker.verify_files(value, roots)
            self.assertEqual(raised.exception.code, "mismatch")
            program.chmod(0o755)
            target = roots["root-a-cold"] / "target"
            target.write_bytes(b"deterministic executable\n")
            target.chmod(0o755)
            (roots["root-a-cold"] / "program").unlink()
            os.symlink(target.name, roots["root-a-cold"] / "program")
            with self.assertRaises(checker.ContractError) as raised:
                checker.verify_files(value, roots)
            self.assertEqual(raised.exception.code, "invalid")

    def test_atomic_output_cleanup_and_fuzz(self):
        raw = (FIXTURES / "two-root-default/evidence.json").read_bytes()
        value = checker.validate(raw)
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "certified.json"
            checker.atomic_write(output, checker.canonical(value))
            self.assertEqual(output.read_bytes(), checker.canonical(value))
            cancelled = Path(directory) / "cancelled.json"
            with self.assertRaises(checker.ContractError) as raised:
                checker.atomic_write(cancelled, b"candidate", cancelled=True)
            self.assertEqual(raised.exception.code, "cancelled")
            self.assertFalse(cancelled.exists())
            self.assertEqual(list(Path(directory).glob(".program-artifacts-*")), [])
        cases, rejected = checker.fuzz(raw, 0.002, 1101)
        self.assertGreater(cases, 0)
        self.assertGreaterEqual(rejected, 0)


if __name__ == "__main__":
    unittest.main()
