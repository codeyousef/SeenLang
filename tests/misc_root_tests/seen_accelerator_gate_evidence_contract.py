#!/usr/bin/env python3
"""Evidence-format safety tests; these never stand in for accelerator execution."""
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import accelerator_gate_evidence as evidence
from accelerator_gate_evidence import publish, validate_checks


class AcceleratorEvidenceContract(unittest.TestCase):
    def setUp(self):
        base = Path(os.environ.get("SEEN_ARTIFACT_ROOT", ROOT / ".seen/agent-tools"))
        base.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="accelerator-evidence-contract.", dir=base)
        self.root = Path(self.temporary.name)
        self.artifacts = self.root / "artifacts"
        self.artifacts.mkdir()
        (self.artifacts / "check.log").write_text("schema fixture, not hardware evidence\n")
        self.row = {"name": "schema-fixture", "exit_status": 0, "classification": "verified",
                    "execution": "native-cpu", "required": True, "logs": ["check.log"], "details": {}}

    def tearDown(self):
        self.temporary.cleanup()

    def rejected(self, row):
        with self.assertRaises(ValueError):
            validate_checks([row], self.artifacts)

    def test_duplicate_check_identity_is_rejected(self):
        with self.assertRaises(ValueError):
            validate_checks([self.row, copy.deepcopy(self.row)], self.artifacts)

    def test_unexecuted_required_result_cannot_pass(self):
        row = {**self.row, "classification": "unsupported", "execution": "none"}
        self.rejected(row)
        self.rejected({**self.row, "execution": "none"})

    def test_native_exit_status_requires_bounded_integer(self):
        for status in (True, -1, 256, "0"):
            with self.subTest(status=status):
                self.rejected({**self.row, "exit_status": status})

    def test_foreign_or_wrong_typed_execution_classification(self):
        for field, value in (("classification", []), ("execution", {}),
                             ("classification", "production-certified"), ("execution", "simulated-gpu")):
            with self.subTest(field=field, value=value):
                self.rejected({**self.row, field: value})

    def test_parent_or_absolute_log_path_is_rejected(self):
        outside = self.root / "outside.log"
        outside.write_text("must not be imported as run evidence")
        for path in ("../outside.log", str(outside)):
            with self.subTest(path=path):
                self.rejected({**self.row, "logs": [path]})

    def test_symlink_log_and_directory_are_rejected(self):
        (self.artifacts / "alias.log").symlink_to(self.artifacts / "check.log")
        self.rejected({**self.row, "logs": ["alias.log"]})
        directory = self.artifacts / "real"
        directory.mkdir()
        (directory / "nested.log").write_text("schema fixture")
        (self.artifacts / "alias").symlink_to(directory, target_is_directory=True)
        self.rejected({**self.row, "logs": ["alias/nested.log"]})

    def test_fifo_rejection_is_nonblocking_and_releases_descriptors(self):
        os.mkfifo(self.artifacts / "pipe")
        before = len(list(Path("/proc/self/fd").iterdir()))
        for _ in range(32):
            self.rejected({**self.row, "logs": ["pipe"]})
        self.assertEqual(len(list(Path("/proc/self/fd").iterdir())), before)

    def test_duplicate_logs_and_directories_are_rejected(self):
        self.rejected({**self.row, "logs": ["check.log", "check.log"]})
        self.rejected({**self.row, "logs": ["."]})

    def test_shared_physical_logs_consume_the_budget_once(self):
        import hashlib
        payload = b"ab"
        (self.artifacts / "check.log").write_bytes(payload)
        os.link(self.artifacts / "check.log", self.artifacts / "alias.log")
        other = {**self.row, "name": "other-check", "logs": ["alias.log"]}
        original = evidence.MAX_LOGS
        evidence.MAX_LOGS = len(payload)
        try:
            observed = validate_checks([self.row, other], self.artifacts)
            digest = {"bytes": 2, "sha256": hashlib.sha256(payload).hexdigest()}
            self.assertEqual([row["logs"] for row in observed],
                             [[{"path": "check.log", **digest}], [{"path": "alias.log", **digest}]])
            (self.artifacts / "distinct.log").write_bytes(b"cd")
            with self.assertRaises(ValueError):
                validate_checks([self.row, {**other, "logs": ["distinct.log"]}], self.artifacts)
        finally:
            evidence.MAX_LOGS = original

    def test_evidence_survives_disposable_run_cleanup_without_build_outputs(self):
        base = self.root / "retained"
        (self.artifacts / "compiler-output").write_bytes(b"not retained")
        shared = self.artifacts / "shared-foundation"
        shared.mkdir()
        (shared / "artifact.spv").write_bytes(b"artifact byte-retention fixture, not GPU execution")
        report = {"phase": "a", "checks": validate_checks([self.row], self.artifacts), "production_certified": False}
        archive = evidence.retain_evidence(self.artifacts, report, base)
        shutil.rmtree(self.artifacts)
        with tarfile.open(archive, "r:gz") as retained:
            self.assertEqual(set(retained.getnames()), {"evidence.json", "logs/check.log", "logs/shared-foundation/artifact.spv"})
            self.assertEqual(retained.extractfile("logs/check.log").read(), b"schema fixture, not hardware evidence\n")
            self.assertEqual(retained.extractfile("logs/shared-foundation/artifact.spv").read(),
                             b"artifact byte-retention fixture, not GPU execution")
            published = json.load(retained.extractfile("evidence.json"))
            self.assertFalse(published["production_certified"])
            self.assertEqual(published["checks"][0]["logs"], report["checks"][0]["logs"])

    def test_retention_prunes_only_indexed_old_evidence_in_its_phase(self):
        base = self.root / "retained"
        archives = []
        for phase, content in (("a", "first"), ("b", "other-phase"), ("a", "second"), ("a", "third")):
            (self.artifacts / "check.log").write_text(content)
            report = {"phase": phase, "checks": validate_checks([self.row], self.artifacts), "production_certified": False}
            archives.append(evidence.retain_evidence(self.artifacts, report, base))
            (base / "user.data").write_text("unrelated data")
        self.assertFalse(archives[0].exists())
        self.assertTrue(all(path.is_file() for path in archives[1:]))
        self.assertEqual((base / "user.data").read_text(), "unrelated data")
        index = json.loads((base / "index.json").read_text())
        self.assertEqual({entry["name"] for entry in index["entries"]}, {path.name for path in archives[1:]})

    def test_native_baseline_witness_survives_collector_and_scratch_cleanup(self):
        import argparse
        import importlib.util
        path = ROOT / "tests/expanded_gate_0_accelerator_certification/runtime_child_evidence.py"
        spec = importlib.util.spec_from_file_location("runtime_child_contract", path)
        collector = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(collector)
        aggregate = self.artifacts / "aggregate"
        aggregate.mkdir()
        (aggregate / "checks.json").write_text("[]")
        (aggregate / "seen_gpu_runtime_a.log").write_text("harness output, not the native witness")
        (aggregate / "seen_gpu_runtime_a.status").write_text("0\n")
        snapshot = aggregate / "before.json"
        snapshot.write_text("[]")
        native = self.artifacts / "gpu-runtime-a.fixture"
        native.mkdir()
        (native / "backend-fast.run.status").write_text("0\n")
        payload = b"native baseline retention fixture, not new GPU execution\n"
        (native / "backend-fast.log").write_bytes(payload)
        args = argparse.Namespace(snapshot=snapshot, prefix="gpu-runtime-a", status=0,
                                  child="seen_gpu_runtime_a", hardware_opt_in="unset", boundary="cpu")
        collector.collect(args, self.artifacts, aggregate)
        checks = validate_checks(json.loads((aggregate / "checks.json").read_text()), aggregate)
        self.assertEqual(checks[0]["execution"], "native-cpu")
        archive = evidence.retain_evidence(aggregate, {"phase": "c", "checks": checks}, self.root / "retained")
        shutil.rmtree(self.artifacts)
        prefix = "logs/children/seen_gpu_runtime_a/gpu-runtime-a.fixture/"
        with tarfile.open(archive, "r:gz") as retained:
            self.assertEqual(retained.extractfile(prefix + "backend-fast.log").read(), payload)
            self.assertEqual(retained.extractfile(prefix + "backend-fast.run.status").read(), b"0\n")

    def test_archive_writer_respects_hard_limit_and_restores_raw_soft_limit(self):
        import subprocess
        code = """
import json, random, resource, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import accelerator_gate_evidence as e
root, base = Path(sys.argv[2]), Path(sys.argv[3])
random_bytes = random.Random(718)
for name in ('left.log', 'right.log'):
    (root / name).write_bytes(random_bytes.randbytes(40 * 1024))
limits = (64 * 1024, e.MAX_ARCHIVE)
resource.setrlimit(resource.RLIMIT_FSIZE, limits)
row = {'name':'file-limit-fixture','exit_status':0,'classification':'compile-only',
       'execution':'official-validation','required':True,'logs':['left.log','right.log'],'details':{}}
checks = e.validate_checks([row], root)
archive = e.retain_evidence(root, {'phase':'a','checks':checks}, base)
assert resource.getrlimit(resource.RLIMIT_FSIZE) == limits
print(archive)
"""
        completed = subprocess.run([sys.executable, "-c", code, str(ROOT / "scripts"),
                                    str(self.artifacts), str(self.root / "retained")],
                                   capture_output=True, text=True, timeout=30)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        archive = Path(completed.stdout.strip())
        self.assertGreater(archive.stat().st_size, 64 * 1024)
        with tarfile.open(archive, "r:gz") as retained:
            self.assertEqual(len(retained.extractfile("logs/left.log").read()), 40 * 1024)
            self.assertEqual(len(retained.extractfile("logs/right.log").read()), 40 * 1024)

    def test_archive_writer_never_raises_an_incompatible_host_hard_limit(self):
        import subprocess
        code = """
import resource, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import accelerator_gate_evidence as e
root, base = Path(sys.argv[2]), Path(sys.argv[3])
limits = (64 * 1024, e.MAX_ARCHIVE - 1)
resource.setrlimit(resource.RLIMIT_FSIZE, limits)
row = {'name':'host-limit-fixture','exit_status':0,'classification':'compile-only',
       'execution':'official-validation','required':True,'logs':['check.log'],'details':{}}
try:
    e.retain_evidence(root, {'phase':'a','checks':e.validate_checks([row],root)}, base)
except ValueError:
    assert resource.getrlimit(resource.RLIMIT_FSIZE) == limits
else:
    raise AssertionError('incompatible host hard limit was ignored')
"""
        completed = subprocess.run([sys.executable, "-c", code, str(ROOT / "scripts"),
                                    str(self.artifacts), str(self.root / "retained")],
                                   capture_output=True, text=True, timeout=30)
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_existing_report_is_not_overwritten(self):
        output = self.artifacts / "report.json"
        output.write_text('{"phase_checks_passed":false}\n')
        before = output.read_bytes()
        with self.assertRaises(ValueError):
            publish(output, {"phase_checks_passed": True})
        self.assertEqual(output.read_bytes(), before)

    def test_nonfinite_json_does_not_publish_partial_report(self):
        output = self.artifacts / "report.json"
        before = set(self.artifacts.iterdir())
        with self.assertRaises(ValueError):
            publish(output, {"measurement": float("nan")})
        self.assertFalse(output.exists())
        self.assertEqual(set(self.artifacts.iterdir()), before)


if __name__ == "__main__":
    unittest.main()
