#!/usr/bin/env python3
"""Filesystem and budget regressions for failed-verification diagnostics."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import unittest
import time
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("verification_logs", ROOT / "scripts/collect_verification_logs.py")
logs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(logs)


class Retention(unittest.TestCase):
    def setUp(self):
        cache = ROOT / ".seen/verification-log-tests"
        cache.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.run = self.root / ".seen/agent-tools/safe-rebuild/run.fixture"
        self.run.mkdir(parents=True)

    def gate(self, child="seen_gpu_runtime_h"):
        gate = self.run / "p0-accel-c.fixture"
        gate.mkdir()
        (gate / "active-child.json").write_text(json.dumps({
            "schema": "seen-accelerator-active-child-v1", "child": child,
            "stage": "running", "child_exit_status": None,
        }) + "\n")
        (gate / (child + ".log")).write_text("still compiling\n")
        (gate / (child + ".status")).write_text("incomplete\n")
        return gate

    def retained(self, result):
        destination = self.root / "target/seen-build/verification-logs"
        return {Path(row["source"]).name: (destination / row["file"]).read_bytes()
                for row in result["files"]}

    def test_tails_and_priority_preserve_the_actual_stop(self):
        (self.run / "safe_rebuild_verify_stage1_acceptance.log").write_bytes(b"old-prefix\n" + b"TIMEOUT: active gate\n")
        leaf = self.run / "gpu.fixture"
        leaf.mkdir()
        (leaf / "00-active.log").write_bytes(b"leaf bytes")
        with patch.object(logs, "MAX_FILE_BYTES", 21), patch.object(logs, "MAX_TOTAL_BYTES", 26):
            result = logs.collect(self.root)
        self.assertEqual(result["retained_bytes"], 26)
        self.assertEqual(result["files"][0]["original_bytes"], 32)
        destination = self.root / "target/seen-build/verification-logs"
        self.assertEqual((destination / "log-000.txt").read_bytes(), b"TIMEOUT: active gate\n")
        self.assertEqual(json.loads((destination / "index.json").read_text()), result)

    def test_file_limit_is_explicit(self):
        for index in range(3):
            (self.run / f"safe_rebuild_{index}.log").write_text("x")
        with patch.object(logs, "MAX_FILES", 2):
            result = logs.collect(self.root)
        self.assertEqual(len(result["files"]), 2)
        self.assertEqual(result["omitted_files"], 1)

    def test_file_symlink_is_rejected(self):
        outside = self.root / "unrelated.txt"
        outside.write_text("not a log")
        (self.run / "safe_rebuild_bad.log").symlink_to(outside)
        with self.assertRaises(OSError):
            logs.collect(self.root)

    def test_parent_symlink_is_rejected(self):
        outside = self.root / "unrelated"
        outside.mkdir()
        (outside / "00-active.log").write_text("not a log")
        (self.run / "gpu.fixture").symlink_to(outside, target_is_directory=True)
        with self.assertRaises(OSError):
            logs.collect(self.root)

    def test_no_logs_still_records_a_bounded_empty_manifest(self):
        result = logs.collect(self.root)
        self.assertEqual(result["files"], [])
        self.assertEqual(result["retained_bytes"], 0)

    def test_nonregular_log_cannot_block_collection(self):
        os.mkfifo(self.run / "safe_rebuild_pipe.log")
        with self.assertRaises(ValueError):
            logs.collect(self.root)

    def test_discovery_has_an_entry_budget(self):
        for index in range(3):
            (self.run / f"safe_rebuild_{index}.log").write_text("x")
        with patch.object(logs, "MAX_ENTRIES", 2), self.assertRaises(ValueError):
            logs.collect(self.root)

    def test_unfinished_gate_keeps_active_identity_and_status_before_old_children(self):
        (self.run / "safe_rebuild_verify.log").write_text("gate scope timed out: 124\n")
        gate = self.gate()
        (gate / "seen_gpu_runtime_a.log").write_text("old child output")
        (gate / "seen_gpu_runtime_a.status").write_text("0\n")
        (gate / "checks.json").write_text('[{"name":"seen_gpu_runtime_a","exit_status":0}]')
        (gate / "seen_gpu_runtime_h.before.json").write_text("[]\n")
        (gate / "seen_gpu_runtime_h.observation.status").write_text("incomplete\n")
        with patch.object(logs, "MAX_FILES", 4):
            result = logs.collect(self.root)
        retained = self.retained(result)
        self.assertEqual(set(retained), {"safe_rebuild_verify.log", "active-child.json",
                                       "seen_gpu_runtime_h.log", "seen_gpu_runtime_h.status"})
        self.assertIn(b"124", retained["safe_rebuild_verify.log"])
        self.assertEqual(retained["seen_gpu_runtime_h.status"], b"incomplete\n")
        marker = next(row for row in result["files"] if Path(row["source"]).name == "active-child.json")
        self.assertEqual(marker["active_child"], "seen_gpu_runtime_h")
        self.assertFalse(marker["incomplete_json"])
        self.assertEqual(result["omitted_files"], 5)

    def test_active_child_survives_a_budget_filled_by_old_build_logs(self):
        self.gate()
        for index in range(8):
            (self.run / f"safe_rebuild_old_{index}.log").write_text("completed build\n")
        with patch.object(logs, "MAX_FILES", 3):
            result = logs.collect(self.root)
        retained = self.retained(result)
        self.assertEqual(set(retained), {
            "active-child.json", "seen_gpu_runtime_h.log", "seen_gpu_runtime_h.status"})
        self.assertEqual(retained["seen_gpu_runtime_h.status"], b"incomplete\n")
        self.assertEqual(result["omitted_files"], 8)

    def test_timeout_status_and_partial_metadata_are_retained_without_success_inference(self):
        gate = self.gate()
        (gate / "seen_gpu_runtime_h.status").write_text("124\n")
        (gate / "seen_gpu_runtime_h.observation.status").write_text("71\n")
        (gate / "checks.json").write_text('[{"name":')
        (gate / "seen_gpu_runtime_h.before.json").write_text("[]\n")
        (gate / "seen_gpu_runtime_h.snapshot.log").write_text("snapshot output\n")
        (gate / "harness-exit.log").write_text("Harness exit status: 137\n")
        result = logs.collect(self.root)
        retained = self.retained(result)
        self.assertEqual(retained["seen_gpu_runtime_h.status"], b"124\n")
        self.assertEqual(retained["seen_gpu_runtime_h.observation.status"], b"71\n")
        self.assertEqual(retained["checks.json"], b'[{"name":')
        self.assertEqual(retained["seen_gpu_runtime_h.before.json"], b"[]\n")
        self.assertIn(b"137", retained["harness-exit.log"])
        checks = next(row for row in result["files"] if Path(row["source"]).name == "checks.json")
        self.assertTrue(checks["incomplete_json"])

    def test_partial_or_unsafe_marker_is_raw_evidence_not_a_path_selector(self):
        for marker in ('{"child":', '{"child":"../../outside"}'):
            with self.subTest(marker=marker):
                gate = self.gate()
                (gate / "active-child.json").write_text(marker)
                result = logs.collect(self.root)
                row = next(row for row in result["files"] if Path(row["source"]).name == "active-child.json")
                self.assertIsNone(row["active_child"])
                self.assertEqual(self.retained(result)["active-child.json"], marker.encode())
                self.assertIn("seen_gpu_runtime_h.log", self.retained(result))
                shutil.rmtree(gate)
                shutil.rmtree(self.root / "target/seen-build/verification-logs")

    def test_gate_allowlist_never_opens_ir_binaries_or_nested_artifacts(self):
        gate = self.gate()
        for name in ("module.ll", "module.bc", "seen_gpu_runtime_h", "seen_gpu_runtime_h.ir",
                     "seen_gpu_runtime_h.spv", "unrelated.log"):
            with (gate / name).open("wb") as output:
                output.truncate(logs.MAX_FILE_BYTES * 8)
        nested = gate / "children"
        nested.mkdir()
        os.mkfifo(nested / "seen_gpu_runtime_h.log")
        (gate / "module-link.ll").symlink_to(self.root / "outside")
        os.mkfifo(gate / "unused.bc")
        read_tail = logs.read_tail
        opened = []
        def observe(root, path, maximum):
            opened.append(path)
            return read_tail(root, path, maximum)
        with patch.object(logs, "read_tail", side_effect=observe):
            result = logs.collect(self.root)
        self.assertEqual(set(self.retained(result)),
                         {"active-child.json", "seen_gpu_runtime_h.log", "seen_gpu_runtime_h.status"})
        self.assertTrue(all(path.name in self.retained(result) for path in opened))

    def test_gate_file_and_total_byte_limits_still_apply(self):
        gate = self.gate()
        (gate / "seen_gpu_runtime_h.log").write_bytes(b"x" * 200 + b"TIMEOUT\n")
        with patch.object(logs, "MAX_FILE_BYTES", 16), patch.object(logs, "MAX_TOTAL_BYTES", 37):
            result = logs.collect(self.root)
        retained = self.retained(result)
        self.assertEqual(result["retained_bytes"], 37)
        self.assertTrue(all(row["retained_bytes"] <= 16 for row in result["files"]))
        self.assertEqual(retained["seen_gpu_runtime_h.log"], b"x" * 8 + b"TIMEOUT\n")
        self.assertEqual(retained["seen_gpu_runtime_h.status"], b"lete\n")
        self.assertTrue(all(row["truncated"] for row in result["files"][:2]))

    def test_gate_discovery_counts_excluded_entries_without_descending(self):
        gate = self.gate()
        (gate / "unused.ll").write_text("not retained")
        # run + gate + four gate entries exceeds five, even for excluded IR.
        with patch.object(logs, "MAX_ENTRIES", 5), self.assertRaises(ValueError):
            logs.collect(self.root)

    def test_gate_symlink_and_fifo_sources_fail_closed(self):
        for name in ("active-child.json", "seen_gpu_runtime_h.log", "seen_gpu_runtime_h.status",
                     "checks.json", "seen_gpu_runtime_h.before.json"):
            for kind in ("symlink", "fifo"):
                with self.subTest(name=name, kind=kind):
                    gate = self.gate()
                    path = gate / name
                    path.unlink(missing_ok=True)
                    if kind == "symlink":
                        outside = self.root / "outside"
                        outside.write_text("must never copy")
                        path.symlink_to(outside)
                    else:
                        os.mkfifo(path)
                    with self.assertRaises((OSError, ValueError)):
                        logs.collect(self.root)
                    shutil.rmtree(gate)
                    shutil.rmtree(self.root / "target/seen-build/verification-logs")

    def test_gate_directory_symlink_fails_closed_before_discovery(self):
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "active-child.json").write_text('{"child":"seen_gpu_runtime_h"}')
        (self.run / "p0-accel-c.fixture").symlink_to(outside, target_is_directory=True)
        with self.assertRaises(OSError):
            logs.collect(self.root)

    def test_mutating_file_is_retained_but_not_claimed_as_a_stable_snapshot(self):
        path = self.run / "safe_rebuild_verify.log"
        path.write_bytes(b"124\n")
        read = os.read
        def mutate(descriptor, maximum):
            content = read(descriptor, maximum)
            with path.open("ab") as output:
                output.write(b"later\n")
            return content
        with patch.object(logs.os, "read", side_effect=mutate):
            result = logs.collect(self.root)
        self.assertEqual(self.retained(result)["safe_rebuild_verify.log"], b"124\n")
        self.assertTrue(result["files"][0]["changed_during_read"])

    def test_disappearing_child_file_does_not_erase_other_diagnostics(self):
        gate = self.gate()
        read_tail = logs.read_tail
        path = gate / "seen_gpu_runtime_h.log"
        def disappear(root, source, maximum):
            if source == path:
                path.unlink()
            return read_tail(root, source, maximum)
        with patch.object(logs, "read_tail", side_effect=disappear):
            result = logs.collect(self.root)
        self.assertEqual(result["unavailable_sources"], [path.relative_to(self.root).as_posix()])
        self.assertIn("active-child.json", self.retained(result))
        self.assertEqual(self.retained(result)["seen_gpu_runtime_h.status"], b"incomplete\n")

    def test_real_driver_publishes_before_a_child_that_never_finishes(self):
        # Exercise the actual shell producer, but replace all compiler/scope/
        # GPU boundaries in an isolated repository. No GPU work is executed.
        fixture = self.root / "driver"
        scripts = fixture / "scripts"
        tests = fixture / "tests/misc_root_tests"
        scripts.mkdir(parents=True)
        tests.mkdir(parents=True)
        driver = tests / "seen_p0_accel_gate_c.sh"
        shutil.copyfile(ROOT / "tests/misc_root_tests/seen_p0_accel_gate_c.sh", driver)
        (scripts / "run_capped_regression.sh").write_text("exit 0\n")
        (scripts / "accelerator_gate_evidence.py").write_text("raise SystemExit(87)\n")
        collector = fixture / "tests/expanded_gate_0_accelerator_certification"
        collector.mkdir()
        (collector / "runtime_child_evidence.py").write_text(
            "import pathlib, sys\n"
            "if '--before' in sys.argv:\n"
            "    pathlib.Path(sys.argv[sys.argv.index('--snapshot') + 1]).write_text('[]')\n"
            "else:\n"
            "    raise SystemExit(71)\n")
        (tests / "seen_gpu_runtime_a.sh").write_text("printf 'first child timeout\\n'\nexit 124\n")
        ready = fixture / "child-ready"
        (tests / "seen_gpu_runtime_a_vulkan.sh").write_text(
            'printf "unfinished child output\\n"\n: > "$CHILD_READY"\nsleep 60\n')
        environment = {**os.environ, "SEEN_CAPPED_REGRESSION_ACTIVE": "1",
                       "SEEN_ARTIFACT_ROOT": str(self.run), "CHILD_READY": str(ready)}
        with (fixture / "driver.log").open("wb") as output:
            process = subprocess.Popen(["bash", str(driver)], env=environment, stdout=output,
                                       stderr=output, start_new_session=True)
            try:
                deadline = time.monotonic() + 10
                while not ready.exists() and process.poll() is None and time.monotonic() < deadline:
                    time.sleep(0.01)
                self.assertTrue(ready.exists(), "synthetic child did not start")
            finally:
                # SIGKILL intentionally prevents the aggregate EXIT report.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
        self.assertEqual(process.returncode, -signal.SIGKILL)
        result = logs.collect(self.root)
        retained = self.retained(result)
        marker = json.loads(retained["active-child.json"])
        self.assertEqual(marker["child"], "seen_gpu_runtime_a_vulkan")
        self.assertEqual(marker["stage"], "running")
        self.assertIsNone(marker["child_exit_status"])
        self.assertEqual(retained["seen_gpu_runtime_a_vulkan.status"], b"incomplete\n")
        self.assertIn(b"unfinished child output", retained["seen_gpu_runtime_a_vulkan.log"])
        self.assertEqual(retained["seen_gpu_runtime_a.status"], b"124\n")
        self.assertEqual(retained["seen_gpu_runtime_a.observation.status"], b"71\n")
        checks = json.loads(retained["checks.json"])
        self.assertEqual(next(row["exit_status"] for row in checks if row["name"] == "seen_gpu_runtime_a"), 124)
        self.assertNotIn("accelerator-c-evidence.json", retained)


if __name__ == "__main__":
    unittest.main()
