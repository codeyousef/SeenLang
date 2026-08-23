#!/usr/bin/env python3

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
        self.assertEqual(report["status"], "passed")

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


if __name__ == "__main__":
    unittest.main()
