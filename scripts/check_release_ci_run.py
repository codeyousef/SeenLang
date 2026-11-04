#!/usr/bin/env python3
"""Verify that a release commit has successful authoritative main CI evidence."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

MAX_BYTES = 1_048_576
MAX_RUNS = 20
SHA = re.compile(r"[0-9a-f]{40}")
REPOSITORY = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+")
ACCEPTANCE_GROUPS = ("core", "io-simd", "gpu-emitter", "gpu-runtime-async")
EXPECTED_JOBS = {"bootstrap", "required", *(f"acceptance ({group})" for group in ACCEPTANCE_GROUPS)}


def load_evidence(raw: bytes) -> object:
    if not 1 <= len(raw) <= MAX_BYTES:
        fail("CI evidence size is outside the bounded range")
    try:
        return json.loads(raw.decode("utf-8"), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"CI evidence is not valid JSON: {error}")


def validate_jobs(raw: bytes, commit: str, run_id: int) -> None:
    value = load_evidence(raw)
    if not isinstance(value, dict) or not isinstance(value.get("jobs"), list):
        fail("CI job evidence is malformed")
    jobs = value["jobs"]
    if value.get("total_count") != len(EXPECTED_JOBS) or len(jobs) != len(EXPECTED_JOBS):
        fail("CI must contain bootstrap, all four acceptance groups and required")
    names = []
    for job in jobs:
        if not isinstance(job, dict) or not isinstance(job.get("name"), str):
            fail("CI job evidence contains a malformed job")
        names.append(job.get("name"))
        if (
            job.get("run_id") != run_id
            or job.get("head_sha") != commit
            or job.get("status") != "completed"
            or job.get("conclusion") != "success"
        ):
            fail("every same-source CI job must complete successfully; skipped jobs are not certification")
    if len(set(names)) != len(names) or set(names) != EXPECTED_JOBS:
        fail("CI job set does not match the authoritative certification graph")


def validate_artifacts(raw: bytes, commit: str, run_id: int) -> None:
    value = load_evidence(raw)
    if not isinstance(value, dict) or not isinstance(value.get("artifacts"), list):
        fail("CI artifact evidence is malformed")
    artifacts = value["artifacts"]
    if value.get("total_count") != len(artifacts) or len(artifacts) > 100:
        fail("CI artifact evidence is incomplete or unbounded")
    eligible = [artifact for artifact in artifacts if isinstance(artifact, dict)
                and artifact.get("name") == f"seen-release-toolchain-{commit}"]
    if len(eligible) != 1:
        fail("exactly one final required release toolchain artifact is required")
    artifact = eligible[0]
    source = artifact.get("workflow_run")
    if (
        artifact.get("expired") is not False
        or not isinstance(artifact.get("size_in_bytes"), int)
        or isinstance(artifact.get("size_in_bytes"), bool)
        or artifact["size_in_bytes"] <= 0
        or not isinstance(source, dict)
        or source.get("id") != run_id
        or source.get("head_sha") != commit
        or source.get("head_branch") != "main"
    ):
        fail("release toolchain artifact is expired, empty or belongs to another source/run")


class EvidenceError(ValueError):
    pass


def fail(message: str) -> None:
    raise EvidenceError(f"core.004b.invalid: {message}")


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            fail(f"duplicate JSON field: {key}")
        value[key] = item
    return value


def validate(raw: bytes, commit: str, repository: str) -> dict[str, object]:
    if not SHA.fullmatch(commit):
        fail("expected commit must be a lowercase 40-character SHA")
    if not REPOSITORY.fullmatch(repository):
        fail("repository identifier is invalid")
    if not 1 <= len(raw) <= MAX_BYTES:
        fail("CI evidence size is outside the bounded range")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"CI evidence is not valid JSON: {error}")
    if not isinstance(value, dict) or set(value) != {"total_count", "workflow_runs"}:
        fail("CI evidence has an unexpected top-level shape")
    runs = value["workflow_runs"]
    if (
        isinstance(value["total_count"], bool)
        or not isinstance(value["total_count"], int)
        or value["total_count"] < 0
        or not isinstance(runs, list)
        or len(runs) > MAX_RUNS
    ):
        fail("CI evidence run count is invalid")

    matches: list[dict[str, object]] = []
    required = {
        "conclusion", "event", "head_branch", "head_sha", "html_url", "id",
        "name", "path", "repository", "status",
    }
    for run in runs:
        if not isinstance(run, dict) or not required.issubset(run):
            fail("CI evidence contains a malformed workflow run")
        source = run["repository"]
        if not isinstance(source, dict) or source.get("full_name") != repository:
            continue
        if (
            run["head_sha"] == commit
            and run["head_branch"] == "main"
            and run["event"] == "push"
            and run["name"] == "CI"
            and run["path"] == ".github/workflows/ci.yml"
            and run["status"] == "completed"
            and run["conclusion"] == "success"
            and isinstance(run["id"], int)
            and not isinstance(run["id"], bool)
            and run["id"] > 0
            and isinstance(run["html_url"], str)
            and run["html_url"]
        ):
            matches.append(run)
    if not matches:
        fail(f"no successful authoritative main CI run exists for {commit}")
    selected = max(matches, key=lambda run: int(run["id"]))
    return {
        "commit": commit,
        "repository": repository,
        "run_id": selected["id"],
        "run_url": selected["html_url"],
        "schema": "seen-release-ci-attestation-v2",
        "status": "selected-unverified",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--jobs-evidence", type=Path)
    parser.add_argument("--artifacts-evidence", type=Path)
    parser.add_argument("--select-run", action="store_true", help="Select a run without attesting its jobs or artifacts")
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repository", required=True)
    args = parser.parse_args()
    try:
        if args.evidence.is_symlink() or not args.evidence.is_file():
            fail("CI evidence path is missing or unsafe")
        report = validate(args.evidence.read_bytes(), args.commit, args.repository)
        if args.select_run:
            if args.jobs_evidence is not None or args.artifacts_evidence is not None:
                fail("run selection cannot consume certification evidence")
            report["status"] = "selected-unverified"
        else:
            if args.jobs_evidence is None or args.artifacts_evidence is None:
                fail("all job and artifact evidence is required for release certification")
            for evidence in (args.jobs_evidence, args.artifacts_evidence):
                if evidence.is_symlink() or not evidence.is_file():
                    fail("CI supplemental evidence path is missing or unsafe")
            validate_jobs(args.jobs_evidence.read_bytes(), args.commit, report["run_id"])
            validate_artifacts(args.artifacts_evidence.read_bytes(), args.commit, report["run_id"])
            report["status"] = "passed"
        print(json.dumps(report, separators=(",", ":"), sort_keys=True))
    except (OSError, EvidenceError) as error:
        print(f"release-ci: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
