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
MAIN_TAG = "v0.22.9"
MAINTENANCE_TAG = "v0.22.12"


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
        report = release_ci.validate(raw, COMMIT, REPOSITORY, MAIN_TAG)
        self.assertEqual(report["run_id"], 42)
        self.assertEqual(report["status"], "passed")

    def test_selects_latest_matching_success(self):
        raw = json.dumps({
            "total_count": 3,
            "workflow_runs": [run(id=41), run(id=43), run(id=42, conclusion="failure")],
        }).encode()
        self.assertEqual(release_ci.validate(raw, COMMIT, REPOSITORY, MAIN_TAG)["run_id"], 43)

    def test_rejects_non_authoritative_runs_on_each_source(self):
        mutations = (
            {"conclusion": "failure"},
            {"conclusion": "cancelled"},
            {"conclusion": None},
            {"event": "pull_request"},
            {"event": "workflow_dispatch"},
            {"head_branch": "feature"},
            {"head_sha": "b" * 40},
            {"name": "Release"},
            {"path": ".github/workflows/release.yml"},
            {"status": "in_progress"},
            {"repository": {"full_name": "fork/SeenLang"}},
        )
        for tag, branch in ((MAIN_TAG, "main"), (MAINTENANCE_TAG, "release/0.22.12")):
            for mutation in mutations:
                with self.subTest(tag=tag, mutation=mutation):
                    candidate = run(head_branch=branch)
                    candidate.update(mutation)
                    raw = json.dumps({"total_count": 1, "workflow_runs": [candidate]}).encode()
                    with self.assertRaises(release_ci.EvidenceError):
                        release_ci.validate(raw, COMMIT, REPOSITORY, tag)

    def test_accepts_exact_maintenance_run_not_main(self):
        raw = json.dumps({
            "total_count": 2,
            "workflow_runs": [run(head_branch="release/0.22.12"), run(id=99)],
        }).encode()
        report = release_ci.validate(raw, COMMIT, REPOSITORY, MAINTENANCE_TAG)
        self.assertEqual(report["run_id"], 42)
        self.assertEqual(report["source_branch"], "release/0.22.12")
        self.assertEqual(report["tag"], MAINTENANCE_TAG)

    def test_branch_authorization_is_exactly_version_bound(self):
        cases = (
            (MAINTENANCE_TAG, "main"),
            (MAIN_TAG, "release/0.22.12"),
            ("v0.22.11", "release/0.22.12"),
            ("v0.22.13", "release/0.22.12"),
            ("v0.23.0", "release/0.22.12"),
            ("v0.22.12-rc.1", "release/0.22.12"),
            (MAINTENANCE_TAG, "release/0.22.11"),
        )
        for tag, branch in cases:
            with self.subTest(tag=tag, branch=branch):
                raw = json.dumps({"total_count": 1, "workflow_runs": [run(head_branch=branch)]}).encode()
                with self.assertRaises(release_ci.EvidenceError):
                    release_ci.validate(raw, COMMIT, REPOSITORY, tag)

    def test_other_versions_keep_main(self):
        raw = json.dumps({"total_count": 1, "workflow_runs": [run()]}).encode()
        for tag in ("v0.22.11", "v0.22.13", "v0.23.0", "v0.22.12-rc.1"):
            with self.subTest(tag=tag):
                self.assertEqual(release_ci.validate(raw, COMMIT, REPOSITORY, tag)["source_branch"], "main")

    def test_probe_and_publication_dispatch_refs(self):
        for tag, branch in ((MAIN_TAG, "main"), (MAINTENANCE_TAG, "release/0.22.12")):
            release_ci.validate_dispatch(tag, f"refs/heads/{branch}", True)
            release_ci.validate_dispatch(tag, f"refs/tags/{tag}", False)
            for ref, probe in (
                (f"refs/heads/{branch}", False),
                (f"refs/tags/{tag}", True),
                ("refs/heads/feature", True),
                ("refs/tags/v0.99.0", False),
            ):
                with self.subTest(tag=tag, ref=ref, probe=probe):
                    with self.assertRaises(release_ci.EvidenceError):
                        release_ci.validate_dispatch(tag, ref, probe)
        with self.assertRaises(release_ci.EvidenceError):
            release_ci.validate_dispatch(MAINTENANCE_TAG, "refs/heads/main", True)
        with self.assertRaises(release_ci.EvidenceError):
            release_ci.validate_dispatch("v0.23.0", "refs/heads/release/0.22.12", True)

    def test_invalid_version_cannot_select_a_branch(self):
        for tag in ("0.22.10", "v00.22.10", "v0.22.10/feature", "v0.22.10\n", ""):
            with self.subTest(tag=tag):
                with self.assertRaises(release_ci.EvidenceError):
                    release_ci.source_branch(tag)

    def test_rejects_malformed_or_unbounded_evidence(self):
        for raw in (b"{}", b"[]", b'{"total_count":1,"total_count":2}', b"x" * (release_ci.MAX_BYTES + 1)):
            with self.assertRaises(release_ci.EvidenceError):
                release_ci.validate(raw, COMMIT, REPOSITORY, MAIN_TAG)

    def test_main_rejects_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "evidence.json"
            target.write_text(json.dumps({"total_count": 1, "workflow_runs": [run()]}))
            link = root / "link.json"
            link.symlink_to(target)
            argv = ["check_release_ci_run.py", "--tag", MAIN_TAG, "--evidence", str(link), "--commit", COMMIT, "--repository", REPOSITORY]
            with mock.patch("sys.argv", argv), redirect_stderr(io.StringIO()):
                self.assertEqual(release_ci.main(), 1)


if __name__ == "__main__":
    unittest.main()
