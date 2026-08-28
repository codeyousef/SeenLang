#!/usr/bin/env python3

import argparse
import copy
import hashlib
import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load("program_checker", ROOT / "scripts/check_program_reproducibility.py")
certifier = load("program_certifier", ROOT / "scripts/certify_two_builder_programs.py")
producer = load(
    "program_evidence_producer",
    ROOT / "scripts/produce_program_reproducibility_evidence.py",
)
FIXTURES = ROOT / "tests/fixtures/core-004g"
HAPPY = FIXTURES / "happy/evidence.json"


def evidence():
    return json.loads(HAPPY.read_text(encoding="utf-8"))


def sha256(payload):
    return hashlib.sha256(payload).hexdigest()


def set_path(value, path, replacement):
    target = value
    for component in path[:-1]:
        target = target[component]
    target[path[-1]] = replacement


def seal_builder(builder):
    builder["attestation"]["inventory_sha256"] = checker.builder_inventory_digest(builder)


def openssl(*arguments):
    return subprocess.run(
        ["openssl", *map(str, arguments)], check=True, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, timeout=30,
    ).stdout


class ProgramReproducibilityTests(unittest.TestCase):
    def reject(self, value, code):
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(json.dumps(value).encode("utf-8"))
        self.assertEqual(raised.exception.code, code)

    def test_happy_is_canonical_and_schema_is_strict(self):
        raw = HAPPY.read_bytes()
        value = checker.validate(raw)
        self.assertEqual(checker.canonical(value), raw)
        self.assertEqual(
            [program["id"] for program in value["programs"]],
            [program_id for program_id, _ in checker.PROGRAMS],
        )
        schema = json.loads(
            (ROOT / "schemas/program-reproducibility.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertFalse(schema["additionalProperties"])
        self.assertEqual(
            schema["properties"]["schema"]["const"], checker.SCHEMA
        )
        self.assertEqual(
            schema["properties"]["programs"]["minItems"], len(checker.PROGRAMS)
        )
        self.assertIn("keyless", schema["$defs"]["attestation"]["properties"]["mode"]["enum"])

    def test_duplicate_invalid_limit_cancel_and_platform(self):
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate((FIXTURES / "invalid/evidence.json").read_bytes())
        self.assertEqual(raised.exception.code, "invalid")
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate((FIXTURES / "limit/evidence.json").read_bytes())
        self.assertEqual(raised.exception.code, "limit")
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(HAPPY.read_bytes(), cancelled=True)
        self.assertEqual(raised.exception.code, "cancelled")
        value = evidence()
        value["target"] = "macos-arm64"
        self.reject(value, "platform")

    def test_named_mutation_fixtures(self):
        names = (
            "raw-mismatch", "builder-identity", "root-identity", "input-drift",
            "toolchain-drift", "installed-archive",
        )
        for name in names:
            with self.subTest(name=name):
                mutation = json.loads(
                    (FIXTURES / name / "mutation.json").read_text(encoding="utf-8")
                )
                value = evidence()
                set_path(value, mutation["path"], mutation["value"])
                seal_builder(value["builders"][mutation["reseal_builder"]])
                self.reject(value, mutation["code"])

    def test_unsigned_behavior_and_name_reuse_fail_closed(self):
        value = evidence()
        value["builders"][0]["attestation"]["identity"] = ""
        self.reject(value, "unsigned")
        value = evidence()
        value["builders"][0]["attestation"]["mode"] = "key"
        self.reject(value, "unsigned")
        value = evidence()
        first_run = value["builders"][0]["programs"][0]["runs"][0]
        first_run["stdout_sha256"] = "7" * 64
        seal_builder(value["builders"][0])
        self.reject(value, "raw_mismatch")
        value = evidence()
        result = value["builders"][0]["programs"][0]
        result["runs"][1]["artifact"] = result["runs"][0]["artifact"]
        seal_builder(value["builders"][0])
        self.reject(value, "invalid")

    def test_json_bounds_and_fuzz(self):
        raw = HAPPY.read_bytes()
        for malformed in (b"{", b"\xff", b"[]"):
            with self.assertRaises(checker.ContractError):
                checker.validate(malformed)
        for maximum in (0, True, checker.MAX_EVIDENCE_BYTES + 1, len(raw) - 1):
            with self.assertRaises(checker.ContractError):
                checker.validate(raw, maximum)
        cases, rejected = checker.fuzz(raw, 0.002, 1101)
        self.assertGreater(cases, 0)
        self.assertGreaterEqual(rejected, 0)

    def prepare_certification(self, directory):
        root = Path(directory)
        build_roots = [root / "builder-a", root / "builder-b"]
        cache_roots = [root / "cache-a", root / "cache-b"]
        signatures = [root / "builder-a.bundle", root / "builder-b.bundle"]
        private_keys = [root / "builder-a.private.pem", root / "builder-b.private.pem"]
        public_keys = [root / "builder-a.public.pem", root / "builder-b.public.pem"]
        identities = []
        for path in build_roots + cache_roots:
            path.mkdir()
        value = evidence()
        compiler_payload = b"pinned compiler"
        archive_payload = b"installed release archive"
        artifact_payload = b"deterministic executable"
        manifest_payload = b'{"deterministic":true}\n'
        stdout_payload = b""
        stderr_payload = b""
        for index, builder in enumerate(value["builders"]):
            builder["build_root_digest"] = certifier.path_identity(
                build_roots[index].resolve()
            )
            builder["cache_root_digest"] = certifier.path_identity(
                cache_roots[index].resolve()
            )
            builder["compiler_sha256"] = sha256(compiler_payload)
            builder["installed_archive_sha256"] = sha256(archive_payload)
            compiler_path = build_roots[index] / builder["compiler"]
            compiler_path.write_bytes(compiler_payload)
            compiler_path.chmod(0o755)
            (build_roots[index] / builder["installed_archive"]).write_bytes(
                archive_payload
            )
            for result in builder["programs"]:
                for run in result["runs"]:
                    artifact = build_roots[index] / run["artifact"]
                    artifact.write_bytes(artifact_payload)
                    artifact.chmod(0o755)
                    (build_roots[index] / run["manifest"]).write_bytes(
                        manifest_payload
                    )
                    (build_roots[index] / run["stdout"]).write_bytes(stdout_payload)
                    (build_roots[index] / run["stderr"]).write_bytes(stderr_payload)
                    run["artifact_sha256"] = sha256(artifact_payload)
                    run["artifact_bytes"] = len(artifact_payload)
                    run["manifest_sha256"] = sha256(manifest_payload)
                    run["manifest_bytes"] = len(manifest_payload)
                    run["stdout_sha256"] = sha256(stdout_payload)
                    run["stdout_bytes"] = len(stdout_payload)
                    run["stderr_sha256"] = sha256(stderr_payload)
                    run["stderr_bytes"] = len(stderr_payload)
            seal_builder(builder)
            openssl("genpkey", "-algorithm", "ED25519", "-out", private_keys[index])
            openssl("pkey", "-in", private_keys[index], "-pubout", "-out", public_keys[index])
            der = openssl("pkey", "-pubin", "-in", public_keys[index], "-outform", "DER")
            identity = "ed25519-sha256:" + sha256(der)
            identities.append(identity)
            builder["attestation"]["mode"] = "key"
            builder["attestation"]["identity"] = identity
            builder["attestation"]["issuer"] = "seenlang-unit-test"
            inventory = root / f"builder-{index}.inventory.json"
            inventory.write_bytes(checker.builder_inventory_payload(builder))
            openssl("pkeyutl", "-sign", "-inkey", private_keys[index],
                    "-rawin", "-in", inventory, "-out", signatures[index])
            builder["attestation"]["signature_sha256"] = sha256(signatures[index].read_bytes())
        candidate = root / "candidate.json"
        candidate.write_bytes(checker.canonical(value))
        args = argparse.Namespace(
            candidate=candidate,
            builder_a_root=build_roots[0],
            builder_b_root=build_roots[1],
            cache_a_root=cache_roots[0],
            cache_b_root=cache_roots[1],
            signature_a=signatures[0],
            signature_b=signatures[1],
            public_key_a=public_keys[0],
            public_key_b=public_keys[1],
            expected_identity_a=identities[0],
            expected_identity_b=identities[1],
            expected_issuer_a="seenlang-unit-test",
            expected_issuer_b="seenlang-unit-test",
            output=root / "certified.json",
            test_cancel_before_commit=False,
        )
        return value, args, build_roots

    def test_certifier_happy_and_atomic_cancel(self):
        with tempfile.TemporaryDirectory() as directory:
            value, args, _ = self.prepare_certification(directory)
            certified = certifier.certify(args)
            self.assertEqual(certified["schema"], checker.SCHEMA)
            self.assertEqual(args.output.read_bytes(), checker.canonical(value))
            self.assertEqual(list(Path(directory).glob(".program-certification-*")), [])
            self.assertEqual(list(Path(directory).glob(".program-ed25519-*")), [])
            args.output = Path(directory) / "cancelled.json"
            args.test_cancel_before_commit = True
            with self.assertRaises(certifier.checker.ContractError) as raised:
                certifier.certify(args)
            self.assertEqual(raised.exception.code, "cancelled")
            self.assertFalse(args.output.exists())
            self.assertEqual(list(Path(directory).glob(".program-certification-*")), [])
            self.assertEqual(list(Path(directory).glob(".program-ed25519-*")), [])

    def test_certifier_rejects_artifact_and_signature_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            _, args, build_roots = self.prepare_certification(directory)
            candidate = checker.validate(args.candidate.read_bytes())
            artifact_name = candidate["builders"][1]["programs"][0]["runs"][0][
                "artifact"
            ]
            artifact = build_roots[1] / artifact_name
            original = artifact.read_bytes()
            artifact.write_bytes(b"drift")
            with self.assertRaises(certifier.checker.ContractError) as raised:
                certifier.certify(args)
            self.assertEqual(raised.exception.code, "raw_mismatch")
            artifact.write_bytes(original)
            artifact.chmod(0o755)
            args.signature_b.write_bytes(b"unsigned replacement")
            with self.assertRaises(certifier.checker.ContractError) as raised:
                certifier.certify(args)
            self.assertEqual(raised.exception.code, "unsigned")

    def test_certifier_rejects_forgery_key_substitution_and_pin_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            value, args, _ = self.prepare_certification(directory)
            signature = args.signature_b.read_bytes()
            args.signature_b.write_bytes(bytes([signature[0] ^ 1]) + signature[1:])
            value["builders"][1]["attestation"]["signature_sha256"] = sha256(
                args.signature_b.read_bytes()
            )
            args.candidate.write_bytes(checker.canonical(value))
            with self.assertRaises(certifier.checker.ContractError) as raised:
                certifier.certify(args)
            self.assertEqual(raised.exception.code, "unsigned")

        with tempfile.TemporaryDirectory() as directory:
            _, args, _ = self.prepare_certification(directory)
            args.public_key_a = args.public_key_b
            with self.assertRaises(certifier.checker.ContractError) as raised:
                certifier.certify(args)
            self.assertEqual(raised.exception.code, "unsigned")

        with tempfile.TemporaryDirectory() as directory:
            value, args, _ = self.prepare_certification(directory)
            args.expected_issuer_a = "wrong-issuer"
            with self.assertRaises(certifier.checker.ContractError) as raised:
                certifier.certify(args)
            self.assertEqual(raised.exception.code, "unsigned")
            value["builders"][0]["attestation"]["mode"] = "keyless"
            args.expected_issuer_a = "seenlang-unit-test"
            args.candidate.write_bytes(checker.canonical(value))
            with self.assertRaises(certifier.checker.ContractError) as raised:
                certifier.certify(args)
            self.assertEqual(raised.exception.code, "unsigned")

    def test_producer_ed25519_output_is_certifier_compatible(self):
        with tempfile.TemporaryDirectory() as directory:
            value, args, _ = self.prepare_certification(directory)
            partial = copy.deepcopy(value["builders"][0])
            partial.pop("attestation")
            root = Path(directory)
            identity = producer.ed25519_public_identity(root / "builder-a.public.pem")
            (root / "trust-pins.json").write_bytes(producer.canonical({
                "builder-a": {
                    "identity": identity,
                    "issuer": "seenlang-local-release-gate",
                    "public_key": "builder-a.public.pem",
                },
                "schema": "seen-program-trust-pins-v1",
            }))
            signed = producer.sign_builder(Path(directory), partial)
            value["builders"][0] = signed
            args.signature_a = Path(directory) / "builder-a.sig"
            args.public_key_a = Path(directory) / "builder-a.public.pem"
            args.expected_identity_a = signed["attestation"]["identity"]
            args.expected_issuer_a = signed["attestation"]["issuer"]
            args.candidate.write_bytes(checker.canonical(value))
            certified = certifier.certify(args)
            self.assertEqual(certified["builders"][0]["attestation"]["mode"], "key")

if __name__ == "__main__":
    unittest.main()
