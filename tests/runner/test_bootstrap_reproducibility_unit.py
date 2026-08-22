#!/usr/bin/env python3
import argparse, copy, importlib.util, io, json, struct, sys, tempfile, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path); module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
checker = load("checker", ROOT / "scripts/check_bootstrap_reproducibility.py")
certifier = load("certifier", ROOT / "scripts/certify_two_builder_bootstrap.py")
FIXTURE = ROOT / "tests/fixtures/core-004a/happy/evidence.json"
def evidence(): return json.loads(FIXTURE.read_text())


class Tests(unittest.TestCase):
    def reject(self, value, code=None):
        with self.assertRaises(checker.ContractError) as raised: checker.validate(json.dumps(value).encode())
        if code: self.assertEqual(raised.exception.code, code)

    def test_happy_canonical_and_primitives(self):
        value = evidence(); self.assertEqual(checker.validate(json.dumps(value).encode())["normalization"], "none")
        self.assertEqual(checker.canonical(value), checker.canonical(value)); self.assertTrue(checker.integer(1)); self.assertFalse(checker.integer(True))
        self.assertTrue(checker.digest("0" * 64, 64)); self.assertFalse(checker.digest("G" * 64, 64)); self.assertTrue(checker.text("seen", 4)); self.assertFalse(checker.text("é", 4))
        self.assertEqual(checker.pairs([("a", 1)]), {"a": 1})
        with self.assertRaises(checker.ContractError): checker.pairs([("a", 1), ("a", 2)])

    def test_json_cancel_and_bounds(self):
        raw = FIXTURE.read_bytes()
        for malformed in (b"{", b"\xff", b'{"schema":1,"schema":2}', b"[]"):
            with self.assertRaises(checker.ContractError): checker.validate(malformed)
        for maximum in (0, True, checker.MAX_BYTES + 1, len(raw) - 1):
            with self.assertRaises(checker.ContractError): checker.validate(raw, maximum)
        with self.assertRaises(checker.ContractError) as raised: checker.validate(raw, cancelled=True)
        self.assertEqual(raised.exception.code, "cancelled")

    def test_top_level_and_limits_matrix(self):
        good = evidence(); cases = {"schema": ["bad"], "target": ["macos"], "source_commit": ["0" * 39],
            "source_digest": ["G" * 64], "source_date_epoch": [True, -1], "normalization": ["strip"], "command": [""]}
        for key, bads in cases.items():
            for bad in bads: value = copy.deepcopy(good); value[key] = bad; self.reject(value)
        value = copy.deepcopy(good); value["extra"] = 1; self.reject(value)
        for bad in (None, {}, {**good["limits"], "extra": 1}): value = copy.deepcopy(good); value["limits"] = bad; self.reject(value)
        for key, bads in {"memory_max_bytes": [0, True], "memory_swap_max_bytes": [1], "pids_max": [0, 25], "jobs": [2], "opt_jobs": [2]}.items():
            for bad in bads: value = copy.deepcopy(good); value["limits"][key] = bad; self.reject(value)
        for bad in (None, [], good["builders"][:1]): value = copy.deepcopy(good); value["builders"] = bad; self.reject(value, "limit")

    def test_builder_matrix(self):
        good = evidence()
        for index in (0, 1):
            for bad in (None, {}, {**good["builders"][index], "extra": 1}): value = copy.deepcopy(good); value["builders"][index] = bad; self.reject(value)
        cases = {"id": ["wrong"], "builder_sha256": ["0" * 63], "build_root_digest": ["G" * 64],
            "raw_artifact_sha256": ["0" * 63], "normalized_artifact_sha256": ["0" * 63],
            "artifact_bytes": [True, 0, checker.MAX_ARTIFACT_BYTES + 1], "toolchain": [""]}
        for key, bads in cases.items():
            for bad in bads: value = copy.deepcopy(good); value["builders"][0][key] = bad; self.reject(value)
        value = copy.deepcopy(good); value["builders"][0]["normalized_artifact_sha256"] = "f" * 64; self.reject(value)
        value = copy.deepcopy(good); value["builders"][1]["builder_sha256"] = "f" * 64; self.reject(value)
        value = copy.deepcopy(good); value["builders"][1]["build_root_digest"] = value["builders"][0]["build_root_digest"]; self.reject(value)
        value = copy.deepcopy(good); value["builders"][1]["toolchain"] = "other"; self.reject(value)
        value = copy.deepcopy(good); value["builders"][1]["raw_artifact_sha256"] = "f" * 64; value["builders"][1]["normalized_artifact_sha256"] = "f" * 64; self.reject(value, "mismatch")
        value = copy.deepcopy(good); value["builders"][1]["artifact_bytes"] += 1; self.reject(value, "mismatch")
        value = copy.deepcopy(good); value["normalization"] = "elf64-x86_64-v1"
        value["builders"][1]["raw_artifact_sha256"] = "f" * 64
        self.assertEqual(checker.validate(json.dumps(value).encode())["normalization"], "elf64-x86_64-v1")

    def certifier_args(self, root):
        artifact_a = root / "artifact-a"; artifact_b = root / "artifact-b"; artifact_a.write_bytes(b"output"); artifact_b.write_bytes(b"output")
        builder_a = root / "builder-a"; builder_b = root / "builder-b"; builder_a.write_bytes(b"pinned-builder"); builder_b.write_bytes(b"pinned-builder")
        return argparse.Namespace(artifact_a=artifact_a, artifact_b=artifact_b, builder_a=builder_a, builder_b=builder_b,
            build_root_digest_a="1" * 64, build_root_digest_b="2" * 64, source_commit="3" * 40,
            source_digest="4" * 64, source_date_epoch=0, toolchain_a="seen-0.10.1 llvm-22",
            toolchain_b="seen-0.10.1 llvm-22", command="seen compile compiler",
            memory_max_bytes=1024, memory_swap_max_bytes=0, pids_max=24, jobs=1, opt_jobs=1,
            normalization="none", output=root / "evidence.json", test_cancel_before_commit=False)

    def synthetic_elf(self, build_id, reverse=False):
        payload = bytearray(600); payload[:6] = b"\x7fELF\x02\x01"; payload[18:20] = b"\x3e\x00"
        struct.pack_into("<Q", payload, 40, 128); struct.pack_into("<HHH", payload, 58, 64, 4, 1)
        names = b"\0.shstrtab\0.note.gnu.build-id\0.rela.dyn\0"; payload[384:384 + len(names)] = names
        note_name = names.index(b".note.gnu.build-id"); rela_name = names.index(b".rela.dyn")
        struct.pack_into("<IIQQQQIIQQ", payload, 128 + 64, 1, 3, 0, 0, 384, len(names), 0, 0, 1, 0)
        struct.pack_into("<IIQQQQIIQQ", payload, 128 + 128, note_name, 7, 0, 0, 432, 36, 0, 0, 4, 0)
        struct.pack_into("<IIQQQQIIQQ", payload, 128 + 192, rela_name, 4, 0, 0, 480, 48, 0, 0, 8, 24)
        struct.pack_into("<III", payload, 432, 4, 20, 3); payload[444:448] = b"GNU\0"; payload[448:468] = build_id
        entries = [struct.pack("<QQq", 16, 8, 3), struct.pack("<QQq", 32, 8, 5)]
        if reverse: entries.reverse()
        payload[480:528] = b"".join(entries)
        return bytes(payload)

    def test_elf_normalization(self):
        first = self.synthetic_elf(b"a" * 20)
        second = self.synthetic_elf(b"b" * 20, reverse=True)
        self.assertNotEqual(first, second)
        self.assertEqual(certifier.elf64_x86_64_v1(first), certifier.elf64_x86_64_v1(second))
        malformed = bytearray(first); malformed[18:20] = b"\xb7\x00"
        with self.assertRaises(certifier.checker.ContractError): certifier.elf64_x86_64_v1(bytes(malformed))

    def test_certifier_happy_cancel_mismatch_and_safety(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); args = self.certifier_args(root); value = certifier.build(args)
            self.assertEqual(value["builders"][0]["raw_artifact_sha256"], value["builders"][1]["raw_artifact_sha256"])
            self.assertEqual(list(root.glob(".two-builder-*")), [])
            args.output = root / "cancel.json"; args.test_cancel_before_commit = True
            with self.assertRaises(certifier.checker.ContractError) as raised: certifier.build(args)
            self.assertEqual(raised.exception.code, "cancelled"); self.assertFalse(args.output.exists()); self.assertEqual(list(root.glob(".two-builder-*")), [])
            args.test_cancel_before_commit = False; args.artifact_b.write_bytes(b"different")
            with self.assertRaises(certifier.checker.ContractError) as raised: certifier.build(args)
            self.assertEqual(raised.exception.code, "mismatch")
            args.artifact_b.unlink(); args.artifact_b.symlink_to(args.artifact_a)
            with self.assertRaises(certifier.checker.ContractError): certifier.build(args)
            args.artifact_b.unlink(); args.artifact_a.write_bytes(self.synthetic_elf(b"a" * 20))
            args.artifact_b.write_bytes(self.synthetic_elf(b"b" * 20, reverse=True)); args.normalization = "elf64-x86_64-v1"
            value = certifier.build(args)
            self.assertNotEqual(value["builders"][0]["raw_artifact_sha256"], value["builders"][1]["raw_artifact_sha256"])
            self.assertEqual(value["builders"][0]["normalized_artifact_sha256"], value["builders"][1]["normalized_artifact_sha256"])

    def test_fuzz_and_main(self):
        cases, rejected = checker.fuzz(FIXTURE.read_bytes(), .002, 1101); self.assertGreater(cases, 0); self.assertGreaterEqual(rejected, 0)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); source = root / "evidence.json"; source.write_bytes(FIXTURE.read_bytes())
            original_stdout, original_stderr = sys.stdout, sys.stderr
            class Output:
                def __init__(self): self.buffer = io.BytesIO()
            try:
                sys.stdout, sys.stderr = Output(), io.StringIO(); self.assertEqual(checker.main(["--evidence", str(source)]), 0)
                self.assertEqual(checker.main(["--evidence", str(source), "--test-cancel-after-read"]), 130)
                self.assertEqual(checker.main(["--evidence", str(source), "--fuzz-seconds", "301"]), 1)
                self.assertEqual(checker.main(["--evidence", str(root / "missing")]), 1)
                link = root / "link"; link.symlink_to(source); self.assertEqual(checker.main(["--evidence", str(link)]), 1)
            finally: sys.stdout, sys.stderr = original_stdout, original_stderr


if __name__ == "__main__": unittest.main()
