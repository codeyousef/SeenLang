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
        "schema": "seen-release-ci-attestation-v1",
        "status": "passed",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repository", required=True)
    args = parser.parse_args()
    try:
        if args.evidence.is_symlink() or not args.evidence.is_file():
            fail("CI evidence path is missing or unsafe")
        report = validate(args.evidence.read_bytes(), args.commit, args.repository)
        print(json.dumps(report, separators=(",", ":"), sort_keys=True))
    except (OSError, EvidenceError) as error:
        print(f"release-ci: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
