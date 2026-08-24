#!/usr/bin/env python3
"""Validate Seen's bounded, deterministic Gate 0 CI workflow contract."""

from __future__ import annotations

import argparse
import json
import os
import signal
import sys
from pathlib import Path

WORKFLOW = ".github/workflows/ci.yml"
RELEASE_WORKFLOW = ".github/workflows/release.yml"
MAX_FILES_HARD = 256
MAX_BYTES_HARD = 4 * 1024 * 1024
EXPECTED_WORKFLOW = """name: CI

on:
  push:
    branches: [main]

permissions:
  contents: read

concurrency:
  group: ci-${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  required:
    name: required
    runs-on: ubuntu-24.04
    timeout-minutes: 210
    env:
      GOTOOLCHAIN: local
      SEEN_RELEASE_CPU_BASELINE: x86-64
    steps:
      - name: Checkout
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
      - name: Set up pinned Go
        uses: actions/setup-go@b7ad1dad31e06c5925ef5d2fc7ad053ef454303e
        with:
          go-version: '1.26.5'
          cache: false
      - name: Provision required isolation and search tools
        run: scripts/provision_ci_host.sh
      - name: Run required contained gates
        run: scripts/run_ci_required.sh
      - name: Prepare verified release toolchain
        run: scripts/prepare_release_toolchain_artifact.sh
      - name: Retain exact-commit release toolchain
        uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a
        with:
          name: seen-release-toolchain-${{ github.sha }}
          path: target/seen-build/release-toolchain.tar.gz
          if-no-files-found: error
          retention-days: 14
          compression-level: 0
""".encode("utf-8")
EXPECTED_RELEASE_WORKFLOW = """name: Release

on:
  push:
    tags: ['v*']

permissions:
  actions: read
  contents: write
  id-token: write

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

jobs:
  release:
    name: signed-release
    runs-on: ubuntu-24.04
    timeout-minutes: 240
    env:
      GH_TOKEN: ${{ github.token }}
      GOTOOLCHAIN: local
      SEEN_JOBS: 1
      SEEN_LOW_MEMORY: 1
      SEEN_OPT_JOBS: 1
      SEEN_RELEASE_CPU_BASELINE: x86-64
      SEEN_RELEASE_SIGN_MODE: keyless
    steps:
      - name: Checkout tag
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
        with:
          fetch-depth: 0
          ref: ${{ github.sha }}
      - name: Verify successful main certification
        run: scripts/verify_release_ci_run.sh
      - name: Set up pinned Go
        uses: actions/setup-go@b7ad1dad31e06c5925ef5d2fc7ad053ef454303e
        with:
          go-version: '1.26.5'
          cache: false
      - name: Install pinned cosign
        uses: sigstore/cosign-installer@6f9f17788090df1f26f669e9d70d6ae9567deba6
        with:
          cosign-release: 'v3.1.3'
      - name: Provision required isolation and search tools
        run: scripts/provision_ci_host.sh
      - name: Build sign and upload release
        run: scripts/build_and_upload_release.sh "${GITHUB_REF_NAME#v}"
""".encode("utf-8")


class WorkflowError(ValueError):
    pass


def fail(code: str, message: str) -> None:
    raise WorkflowError(f"core.001a.{code}: {message}")


def positive_bounded(value: str, maximum: int, name: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise argparse.ArgumentTypeError(f"{name} must be an integer") from error
    if parsed < 1 or parsed > maximum:
        raise argparse.ArgumentTypeError(f"{name} must be between 1 and {maximum}")
    return parsed


def safe_github_files(
    root: Path, max_files: int, max_bytes: int, cancel_after: int
) -> dict[str, bytes]:
    github = root / ".github"
    if not github.is_dir() or github.is_symlink():
        fail("invalid", "missing or unsafe .github directory")

    entries = sorted(
        github.rglob("*"), key=lambda path: path.relative_to(root).as_posix().encode()
    )
    files: dict[str, bytes] = {}
    total_bytes = 0
    for entry in entries:
        relative = entry.relative_to(root).as_posix()
        if entry.is_symlink():
            fail("invalid", f"symbolic link is not allowed: {relative}")
        if entry.is_dir():
            if entry.name == "workflows-disabled":
                fail("invalid", f"retired workflow directory remains: {relative}")
            continue
        if not entry.is_file():
            fail("invalid", f"non-regular .github entry: {relative}")
        if len(files) >= max_files:
            fail("limit", f".github file limit exceeded ({max_files})")
        if relative.endswith(".disabled"):
            fail("invalid", f"retired workflow remains: {relative}")
        resolved = entry.resolve(strict=True)
        try:
            resolved.relative_to(root)
        except ValueError:
            fail("invalid", f".github entry escaped repository root: {relative}")
        data = entry.read_bytes()
        total_bytes += len(data)
        if total_bytes > max_bytes:
            fail("limit", f".github byte limit exceeded ({max_bytes})")
        try:
            data.decode("utf-8")
        except UnicodeDecodeError as error:
            fail("invalid", f"non-UTF-8 .github file: {relative}: {error}")
        files[relative] = data
        if cancel_after and len(files) >= cancel_after:
            raise KeyboardInterrupt
    return files


def mismatch_line(actual: bytes, expected: bytes) -> int:
    actual_lines = actual.splitlines()
    expected_lines = expected.splitlines()
    for number, (left, right) in enumerate(zip(actual_lines, expected_lines), 1):
        if left != right:
            return number
    return min(len(actual_lines), len(expected_lines)) + 1


def validate(root: Path, max_files: int, max_bytes: int, cancel_after: int) -> dict[str, object]:
    files = safe_github_files(root, max_files, max_bytes, cancel_after)
    workflows = sorted(
        path
        for path in files
        if path.startswith(".github/workflows/") and path.endswith((".yml", ".yaml"))
    )
    if workflows != [WORKFLOW, RELEASE_WORKFLOW]:
        fail(
            "invalid",
            f"active workflow set must be exactly {WORKFLOW} and {RELEASE_WORKFLOW}",
        )
    actual = files[WORKFLOW]
    if actual != EXPECTED_WORKFLOW:
        fail(
            "invalid",
            f"{WORKFLOW} differs from the canonical contract at line "
            f"{mismatch_line(actual, EXPECTED_WORKFLOW)}",
        )
    release_actual = files[RELEASE_WORKFLOW]
    if release_actual != EXPECTED_RELEASE_WORKFLOW:
        fail(
            "invalid",
            f"{RELEASE_WORKFLOW} differs from the canonical contract at line "
            f"{mismatch_line(release_actual, EXPECTED_RELEASE_WORKFLOW)}",
        )
    return {
        "active_workflows": workflows,
        "checkout": "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
        "gate": "scripts/run_ci_required.sh",
        "go": "1.26.5",
        "host_tools": [
            "bubblewrap",
            "clang-20",
            "libclang-rt-20-dev",
            "libvulkan-dev",
            "lld-20",
            "llvm-20",
            "ripgrep",
        ],
        "host_tools_provisioning": "ubuntu-packages-scoped-apparmor",
        "setup_go": "actions/setup-go@b7ad1dad31e06c5925ef5d2fc7ad053ef454303e",
        "job_count": 1,
        "platforms": {
            "linux-arm64": "static-policy",
            "linux-x86_64": "required",
            "macos": "static-policy",
            "windows": "static-policy",
        },
        "required_check": "CI / required",
        "release_cosign": "v3.1.3",
        "release_ci_attestation": "scripts/verify_release_ci_run.sh",
        "release_checkout_ref": "explicit-event-commit-sha",
        "release_cpu_baseline": "x86-64",
        "release_signing": "github-oidc-keyless",
        "release_toolchain_artifact": "exact-main-run",
        "release_workflow": RELEASE_WORKFLOW,
        "runner": "ubuntu-24.04",
        "timeout_minutes": 210,
        "trigger": "push-main-only",
        "upload_artifact": "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
        "version": 6,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--max-files", default="64")
    parser.add_argument("--max-bytes", default=str(1024 * 1024))
    parser.add_argument("--test-cancel-after-files", type=int, default=0, help=argparse.SUPPRESS)
    args = parser.parse_args()

    try:
        max_files = positive_bounded(args.max_files, MAX_FILES_HARD, "max-files")
        max_bytes = positive_bounded(args.max_bytes, MAX_BYTES_HARD, "max-bytes")
        if args.test_cancel_after_files:
            if os.environ.get("SEEN_CI_WORKFLOW_TEST_HOOKS") != "1":
                fail("invalid", "cancellation test hook is disabled")
            if args.test_cancel_after_files < 1:
                fail("invalid", "cancellation test hook must be positive")
        root = args.root.resolve(strict=True)
        if not root.is_dir():
            fail("invalid", "repository root is not a directory")
        report = validate(root, max_files, max_bytes, args.test_cancel_after_files)
        sys.stdout.buffer.write(
            (json.dumps(report, indent=2, ensure_ascii=False, sort_keys=True) + "\n").encode()
        )
    except KeyboardInterrupt:
        print("ci-workflows: core.001a.cancelled: validation cancelled", file=sys.stderr)
        return 130
    except (OSError, WorkflowError, argparse.ArgumentTypeError) as error:
        print(f"ci-workflows: {error}", file=sys.stderr)
        return 1
    return 0


def cancel(_signum: int, _frame: object) -> None:
    raise KeyboardInterrupt


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, cancel)
    sys.exit(main())
