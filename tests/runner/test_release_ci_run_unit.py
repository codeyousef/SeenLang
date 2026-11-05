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
MAIN_TAG = "v0.22.9"
MAINTENANCE_TAG = "v0.22.11"


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
        self.assertEqual(report["status"], "selected-unverified")

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
        for tag, branch in ((MAIN_TAG, "main"), (MAINTENANCE_TAG, "release/0.22.11")):
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
            "workflow_runs": [run(head_branch="release/0.22.11"), run(id=99)],
        }).encode()
        report = release_ci.validate(raw, COMMIT, REPOSITORY, MAINTENANCE_TAG)
        self.assertEqual(report["run_id"], 42)
        self.assertEqual(report["source_branch"], "release/0.22.11")
        self.assertEqual(report["tag"], MAINTENANCE_TAG)

    def test_branch_authorization_is_exactly_version_bound(self):
        cases = (
            (MAINTENANCE_TAG, "main"),
            (MAIN_TAG, "release/0.22.11"),
            ("v0.22.10", "release/0.22.11"),
            ("v0.22.12", "release/0.22.11"),
            ("v0.23.0", "release/0.22.11"),
            ("v0.23.0", "release/0.23.0"),
            (MAINTENANCE_TAG, "release/0.23.0"),
            ("v0.22.11-rc.1", "release/0.22.11"),
            (MAINTENANCE_TAG, "release/0.22.10"),
        )
        for tag, branch in cases:
            with self.subTest(tag=tag, branch=branch):
                raw = json.dumps({"total_count": 1, "workflow_runs": [run(head_branch=branch)]}).encode()
                with self.assertRaises(release_ci.EvidenceError):
                    release_ci.validate(raw, COMMIT, REPOSITORY, tag)

    def test_other_versions_keep_main(self):
        raw = json.dumps({"total_count": 1, "workflow_runs": [run()]}).encode()
        for tag in ("v0.22.10", "v0.22.12", "v0.23.0", "v0.22.11-rc.1"):
            with self.subTest(tag=tag):
                self.assertEqual(release_ci.validate(raw, COMMIT, REPOSITORY, tag)["source_branch"], "main")

    def test_probe_and_publication_dispatch_refs(self):
        for tag, branch in ((MAIN_TAG, "main"), (MAINTENANCE_TAG, "release/0.22.11")):
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
            release_ci.validate_dispatch("v0.23.0", "refs/heads/release/0.22.11", True)
        with self.assertRaises(release_ci.EvidenceError):
            release_ci.validate_dispatch("v0.23.0", "refs/heads/release/0.23.0", True)

    def test_invalid_version_cannot_select_a_branch(self):
        for tag in ("0.22.10", "v00.22.10", "v0.22.10/feature", "v0.22.10\n", ""):
            with self.subTest(tag=tag):
                with self.assertRaises(release_ci.EvidenceError):
                    release_ci.source_branch(tag)

    def test_policy_cli_matches_release_shell_and_windows_dispatch_callers(self):
        from contextlib import redirect_stdout

        for tag, branch in (
            (MAIN_TAG, "main"),
            (MAINTENANCE_TAG, "release/0.22.11"),
            ("v0.23.0", "main"),
        ):
            base = ["check_release_ci_run.py", "--tag", tag]
            stream = io.StringIO()
            with self.subTest(tag=tag), mock.patch("sys.argv", base + ["--source-branch"]), redirect_stdout(stream):
                self.assertEqual(release_ci.main(), 0)
            self.assertEqual(stream.getvalue().strip(), branch)
            for options in (
                ["--dispatch-ref", f"refs/heads/{branch}", "--probe-only"],
                ["--dispatch-ref", f"refs/tags/{tag}"],
            ):
                with self.subTest(tag=tag, options=options), mock.patch("sys.argv", base + options):
                    self.assertEqual(release_ci.main(), 0)
            for options in (
                ["--dispatch-ref", f"refs/heads/{branch}"],
                ["--dispatch-ref", f"refs/tags/{tag}", "--probe-only"],
                ["--dispatch-ref", "refs/heads/release/0.23.0", "--probe-only"],
            ):
                with self.subTest(tag=tag, options=options), mock.patch("sys.argv", base + options), redirect_stderr(io.StringIO()):
                    self.assertEqual(release_ci.main(), 1)

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

    def jobs(self):
        return {
            "total_count": 6,
            "jobs": [
                {"name": name, "run_id": 42, "head_sha": COMMIT,
                 "status": "completed", "conclusion": "success"}
                for name in sorted(release_ci.EXPECTED_JOBS)
            ],
        }

    def artifacts(self, branch="main"):
        return {
            "total_count": 1,
            "artifacts": [{
                "name": f"seen-release-toolchain-{COMMIT}",
                "expired": False, "size_in_bytes": 128,
                "workflow_run": {"id": 42, "head_sha": COMMIT, "head_branch": branch},
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
        release_ci.validate_artifacts(json.dumps(evidence).encode(), COMMIT, 42, MAIN_TAG)
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
                release_ci.validate_artifacts(json.dumps(invalid).encode(), COMMIT, 42, MAIN_TAG)
        for count in (0, 2):
            invalid = self.artifacts()
            invalid["artifacts"] *= count
            invalid["total_count"] = count
            with self.assertRaises(release_ci.EvidenceError):
                release_ci.validate_artifacts(json.dumps(invalid).encode(), COMMIT, 42, MAIN_TAG)

    def test_final_artifact_authorization_is_exactly_tag_bound(self):
        for tag, branch in (
            (MAIN_TAG, "main"),
            (MAINTENANCE_TAG, "release/0.22.11"),
            ("v0.23.0", "main"),
            ("v0.22.11-rc.1", "main"),
        ):
            with self.subTest(tag=tag, branch=branch):
                release_ci.validate_artifacts(json.dumps(self.artifacts(branch)).encode(), COMMIT, 42, tag)
            for other_branch in {"main", "release/0.22.11", "release/0.23.0"} - {branch}:
                with self.subTest(tag=tag, branch=other_branch), self.assertRaises(release_ci.EvidenceError):
                    release_ci.validate_artifacts(
                        json.dumps(self.artifacts(other_branch)).encode(), COMMIT, 42, tag
                    )

    def test_cli_requires_complete_evidence_and_selection_is_not_attestation(self):
        from contextlib import redirect_stdout

        for tag, branch in (("v0.23.0", "main"), (MAINTENANCE_TAG, "release/0.22.11")):
            with self.subTest(tag=tag), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                evidence = root / "runs.json"
                evidence.write_text(json.dumps({
                    "total_count": 1, "workflow_runs": [run(head_branch=branch)],
                }))
                jobs = root / "jobs.json"
                jobs.write_text(json.dumps(self.jobs()))
                artifacts = root / "artifacts.json"
                artifacts.write_text(json.dumps(self.artifacts(branch)))
                base = ["check_release_ci_run.py", "--tag", tag, "--evidence", str(evidence),
                        "--commit", COMMIT, "--repository", REPOSITORY]
                with mock.patch("sys.argv", base), redirect_stderr(io.StringIO()):
                    self.assertEqual(release_ci.main(), 1)
                stream = io.StringIO()
                with mock.patch("sys.argv", base + ["--select-run"]), redirect_stdout(stream):
                    self.assertEqual(release_ci.main(), 0)
                self.assertEqual(json.loads(stream.getvalue())["status"], "selected-unverified")
                stream = io.StringIO()
                complete = base + ["--jobs-evidence", str(jobs), "--artifacts-evidence", str(artifacts)]
                with mock.patch("sys.argv", complete), redirect_stdout(stream):
                    self.assertEqual(release_ci.main(), 0)
                report = json.loads(stream.getvalue())
                self.assertEqual(report["status"], "passed")
                self.assertEqual(report["source_branch"], branch)
                self.assertEqual(report["tag"], tag)
                with mock.patch("sys.argv", complete + ["--select-run"]), redirect_stderr(io.StringIO()):
                    self.assertEqual(release_ci.main(), 1)
                artifacts.write_text(json.dumps(self.artifacts(
                    "main" if branch != "main" else "release/0.23.0"
                )))
                with mock.patch("sys.argv", complete), redirect_stderr(io.StringIO()):
                    self.assertEqual(release_ci.main(), 1)


if __name__ == "__main__":
    unittest.main()
