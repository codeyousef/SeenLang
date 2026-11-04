#!/usr/bin/env python3

import copy
import importlib.util
import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "release_ci", ROOT / "scripts/check_release_ci_run.py"
)
release_ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release_ci)

COMMIT = "a" * 40
REPOSITORY = "codeyousef/SeenLang"


def run(**changes):
    value = {
        "conclusion": "success",
        "event": "push",
        "head_branch": "main",
        "head_sha": COMMIT,
        "html_url": "https://github.com/codeyousef/SeenLang/actions/runs/42",
        "id": 42,
        "name": "CI",
        "path": ".github/workflows/ci.yml",
        "repository": {"full_name": REPOSITORY},
        "status": "completed",
    }
    value.update(changes)
    return value


class Tests(unittest.TestCase):
    def test_accepts_exact_successful_main_run(self):
        raw = json.dumps({"total_count": 1, "workflow_runs": [run()]}).encode()
        report = release_ci.validate(raw, COMMIT, REPOSITORY)
        self.assertEqual(report["run_id"], 42)
        self.assertEqual(report["status"], "selected-unverified")

    def test_selects_latest_matching_success(self):
        raw = json.dumps({
            "total_count": 3,
            "workflow_runs": [run(id=41), run(id=43), run(id=42, conclusion="failure")],
        }).encode()
        self.assertEqual(release_ci.validate(raw, COMMIT, REPOSITORY)["run_id"], 43)

    def test_rejects_non_authoritative_runs(self):
        mutations = (
            {"conclusion": "failure"},
            {"event": "pull_request"},
            {"head_branch": "feature"},
            {"head_sha": "b" * 40},
            {"name": "Release"},
            {"path": ".github/workflows/release.yml"},
            {"status": "in_progress"},
            {"repository": {"full_name": "fork/SeenLang"}},
        )
        for mutation in mutations:
            raw = json.dumps({"total_count": 1, "workflow_runs": [run(**mutation)]}).encode()
            with self.assertRaises(release_ci.EvidenceError):
                release_ci.validate(raw, COMMIT, REPOSITORY)

    def test_rejects_malformed_or_unbounded_evidence(self):
        for raw in (b"{}", b"[]", b'{"total_count":1,"total_count":2}', b"x" * (release_ci.MAX_BYTES + 1)):
            with self.assertRaises(release_ci.EvidenceError):
                release_ci.validate(raw, COMMIT, REPOSITORY)

    def test_main_rejects_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "evidence.json"
            target.write_text(json.dumps({"total_count": 1, "workflow_runs": [run()]}))
            link = root / "link.json"
            link.symlink_to(target)
            argv = ["check_release_ci_run.py", "--evidence", str(link), "--commit", COMMIT, "--repository", REPOSITORY]
            with mock.patch("sys.argv", argv), redirect_stderr(io.StringIO()):
                self.assertEqual(release_ci.main(), 1)

    def jobs(self):
        return {
            "total_count": 6,
            "jobs": [
                {"name": name, "run_id": 42, "head_sha": COMMIT,
                 "status": "completed", "conclusion": "success"}
                for name in sorted(release_ci.EXPECTED_JOBS)
            ],
        }

    def artifacts(self):
        return {
            "total_count": 1,
            "artifacts": [{
                "name": f"seen-release-toolchain-{COMMIT}",
                "expired": False, "size_in_bytes": 128,
                "workflow_run": {"id": 42, "head_sha": COMMIT, "head_branch": "main"},
            }],
        }

    def test_all_six_same_source_jobs_are_required(self):
        jobs = self.jobs()
        release_ci.validate_jobs(json.dumps(jobs).encode(), COMMIT, 42)
        for index in range(6):
            for conclusion in ("failure", "skipped", "cancelled", "timed_out", None):
                invalid = copy.deepcopy(jobs)
                invalid["jobs"][index]["conclusion"] = conclusion
                with self.subTest(index=index, conclusion=conclusion), self.assertRaises(release_ci.EvidenceError):
                    release_ci.validate_jobs(json.dumps(invalid).encode(), COMMIT, 42)
        for field, value in (("run_id", 43), ("head_sha", "b" * 40),
                             ("status", "in_progress"), ("name", "unexpected"), ("name", {})):
            invalid = copy.deepcopy(jobs)
            invalid["jobs"][0][field] = value
            with self.subTest(field=field), self.assertRaises(release_ci.EvidenceError):
                release_ci.validate_jobs(json.dumps(invalid).encode(), COMMIT, 42)

    def test_missing_duplicate_and_paginated_jobs_are_rejected(self):
        for change in ("missing", "duplicate", "extra-page"):
            jobs = self.jobs()
            if change == "missing":
                jobs["jobs"].pop()
                jobs["total_count"] = 5
            elif change == "duplicate":
                jobs["jobs"][0] = jobs["jobs"][1]
            else:
                jobs["total_count"] = 7
            with self.subTest(change=change), self.assertRaises(release_ci.EvidenceError):
                release_ci.validate_jobs(json.dumps(jobs).encode(), COMMIT, 42)

    def test_only_live_same_run_final_artifact_is_eligible(self):
        evidence = self.artifacts()
        release_ci.validate_artifacts(json.dumps(evidence).encode(), COMMIT, 42)
        for changes in (
            {"name": f"seen-bootstrap-candidate-{COMMIT}"},
            {"name": f"seen-acceptance-receipt-core-{COMMIT}"},
            {"expired": True}, {"size_in_bytes": 0},
            {"workflow_run": {"id": 43, "head_sha": COMMIT, "head_branch": "main"}},
            {"workflow_run": {"id": 42, "head_sha": "b" * 40, "head_branch": "main"}},
            {"workflow_run": {"id": 42, "head_sha": COMMIT, "head_branch": "release/0.23.0"}},
        ):
            invalid = copy.deepcopy(evidence)
            invalid["artifacts"][0].update(changes)
            with self.subTest(changes=changes), self.assertRaises(release_ci.EvidenceError):
                release_ci.validate_artifacts(json.dumps(invalid).encode(), COMMIT, 42)
        for count in (0, 2):
            invalid = self.artifacts()
            invalid["artifacts"] *= count
            invalid["total_count"] = count
            with self.assertRaises(release_ci.EvidenceError):
                release_ci.validate_artifacts(json.dumps(invalid).encode(), COMMIT, 42)

    def test_cli_requires_complete_evidence_and_selection_is_not_attestation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = root / "runs.json"
            evidence.write_text(json.dumps({"total_count": 1, "workflow_runs": [run()]}))
            jobs = root / "jobs.json"
            jobs.write_text(json.dumps(self.jobs()))
            artifacts = root / "artifacts.json"
            artifacts.write_text(json.dumps(self.artifacts()))
            base = ["check_release_ci_run.py", "--evidence", str(evidence),
                    "--commit", COMMIT, "--repository", REPOSITORY]
            with mock.patch("sys.argv", base), redirect_stderr(io.StringIO()):
                self.assertEqual(release_ci.main(), 1)
            from contextlib import redirect_stdout
            stream = io.StringIO()
            with mock.patch("sys.argv", base + ["--select-run"]), redirect_stdout(stream):
                self.assertEqual(release_ci.main(), 0)
            self.assertEqual(json.loads(stream.getvalue())["status"], "selected-unverified")
            stream = io.StringIO()
            complete = base + ["--jobs-evidence", str(jobs), "--artifacts-evidence", str(artifacts)]
            with mock.patch("sys.argv", complete), redirect_stdout(stream):
                self.assertEqual(release_ci.main(), 0)
            self.assertEqual(json.loads(stream.getvalue())["status"], "passed")


if __name__ == "__main__":
    unittest.main()
