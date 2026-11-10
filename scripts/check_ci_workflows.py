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
ACCEPTANCE_GROUPS = ("core", "io-simd", "gpu-emitter", "gpu-runtime", "async")
CHECKOUT = "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683"
SETUP_GO = "actions/setup-go@b7ad1dad31e06c5925ef5d2fc7ad053ef454303e"
UPLOAD_ARTIFACT = "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
# Official actions/download-artifact refs/tags/v4.3.0, resolved through GitHub's API.
DOWNLOAD_ARTIFACT = "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093"
SOURCE_SHA = "${{ github.sha }}"
MATRIX_GROUP = "${{ matrix.group }}"
DIAGNOSTIC_IF = "${{ !cancelled() }}"
JOB_CLOCK = ('IFS=" " read -r uptime _ < /proc/uptime\n'
             'printf \'SEEN_CI_JOB_STARTED_UPTIME_SECS=%s\\n\' "${uptime%%.*}" >> "$GITHUB_ENV"\n')


def provision_steps() -> list[dict]:
    return [
        {"if": DIAGNOSTIC_IF, "timeout-minutes": 1, "run": JOB_CLOCK},
        {"uses": CHECKOUT, "with": {"ref": SOURCE_SHA}},
        {"uses": SETUP_GO, "with": {"go-version": "1.26.5", "cache": False}},
        {"run": "scripts/provision_ci_host.sh"},
    ]


def artifact_upload(name: str, path: str, *, days: int = 14, compression: int = 6,
                    missing: str = "error", diagnostic: bool = False,
                    hidden: bool = False) -> dict:
    step = {"uses": UPLOAD_ARTIFACT, "with": {
        "name": name, "path": path, "if-no-files-found": missing,
        "retention-days": days, "compression-level": compression,
    }}
    if diagnostic:
        step["if"] = DIAGNOSTIC_IF
        step["timeout-minutes"] = 1
    if hidden:
        step["with"]["include-hidden-files"] = True
    return step


def diagnostics(phase: str) -> list[dict]:
    return [
        {"if": DIAGNOSTIC_IF, "timeout-minutes": 1,
         "run": "python3 scripts/collect_verification_logs.py"},
        artifact_upload(f"seen-verification-logs-{phase}-{SOURCE_SHA}",
                        "target/seen-build/verification-logs/", days=7, diagnostic=True),
        artifact_upload(f"seen-accelerator-evidence-{phase}-{SOURCE_SHA}",
                        ".seen/accelerator-gate-evidence/*.tar.gz\n"
                        ".seen/accelerator-gate-evidence/index.json\n",
                        compression=0, missing="ignore", diagnostic=True, hidden=True),
        artifact_upload(f"seen-io-overlap-{phase}-{SOURCE_SHA}",
                        ".seen/agent-tools/safe-rebuild/run.*/io-overlap-evidence.*/*.json",
                        missing="ignore", diagnostic=True, hidden=True),
    ]


def candidate_steps() -> list[dict]:
    return [
        {"uses": DOWNLOAD_ARTIFACT, "with": {
            "name": f"seen-bootstrap-candidate-{SOURCE_SHA}",
            "path": "target/seen-build/candidate-transfer/",
        }},
        {"run": 'python3 scripts/ci_bootstrap_candidate.py unpack --root "$PWD" '
                '--archive "$PWD/target/seen-build/candidate-transfer/bootstrap-candidate.tar.gz" '
                '--output "$PWD/target/seen-build/bootstrap-candidate"'},
    ]


def hosted_job(name: str, minutes: int, steps: list[dict], **fields: object) -> dict:
    return {"name": name, "runs-on": "ubuntu-24.04", "timeout-minutes": minutes,
            "env": {"GOTOOLCHAIN": "local", "SEEN_RELEASE_CPU_BASELINE": "x86-64"},
            **fields, "steps": steps}


def ci_policy() -> dict:
    bootstrap = hosted_job("bootstrap", 120, provision_steps() + [
        {"run": "scripts/run_ci_required.sh --phase bootstrap"},
        {"timeout-minutes": 1, "run": "tar -czf target/seen-build/bootstrap-candidate.tar.gz "
                "-C target/seen-build/bootstrap-candidate seen seen-pkg candidate.json"},
        {**artifact_upload(f"seen-bootstrap-candidate-{SOURCE_SHA}",
                           "target/seen-build/bootstrap-candidate.tar.gz", compression=0),
         "timeout-minutes": 1},
    ] + diagnostics("bootstrap"))
    acceptance_provision = provision_steps()
    acceptance_provision[-1]["env"] = {
        "SEEN_PREPARE_FINITE_TSAN_SDK":
            "${{ (matrix.group == 'core' || matrix.group == 'async') && '1' || '0' }}",
    }
    acceptance = hosted_job("acceptance (${{ matrix.group }})", 195,
        acceptance_provision + candidate_steps() + [
            {"run": f"scripts/run_ci_required.sh --phase acceptance --group {MATRIX_GROUP}"},
            {**artifact_upload(f"seen-acceptance-receipt-{MATRIX_GROUP}-{SOURCE_SHA}",
                               f"target/seen-build/acceptance-receipts/{MATRIX_GROUP}.json"),
             "timeout-minutes": 1},
        ] + diagnostics(f"acceptance-{MATRIX_GROUP}"), needs="bootstrap",
        strategy={"fail-fast": False, "matrix": {"group": list(ACCEPTANCE_GROUPS)}})
    native_inputs = hosted_job("native-arm-inputs", 145, provision_steps() + [
        {"run": "sudo -n apt-get install --yes --no-install-recommends "
                "gcc-aarch64-linux-gnu g++-aarch64-linux-gnu"},
    ] + candidate_steps() + [
        {"run": "scripts/run_ci_required.sh --phase native-arm-inputs"},
        {**artifact_upload(f"seen-linux-arm64-inputs-{SOURCE_SHA}",
                           "target/seen-build/linux-arm64-inputs.tar.gz", compression=0),
         "timeout-minutes": 1},
    ] + diagnostics("native-arm-inputs"), needs="bootstrap", env={
        "GOTOOLCHAIN": "local", "SEEN_RELEASE_CPU_BASELINE": "x86-64",
        "SEEN_LINUX_ARM64_SYSROOT": "/",
        "SEEN_LINUX_ARM64_GCC_TOOLCHAIN": "/usr",
    })
    native_provision = provision_steps()
    native_provision[-1]["run"] = "scripts/provision_ci_linux_arm64_host.sh"
    native_archive = {"uses": DOWNLOAD_ARTIFACT, "with": {
        "name": f"seen-linux-arm64-inputs-{SOURCE_SHA}",
        "path": "target/seen-build/linux-arm64-transfer/",
    }}
    native = hosted_job("native-arm", 50, native_provision + candidate_steps() + [
        native_archive,
        {"run": "scripts/run_ci_required.sh --phase native-arm"},
        {**artifact_upload(f"seen-linux-arm64-qualification-{SOURCE_SHA}",
                           "target/seen-build/linux-arm64-qualification/"),
         "timeout-minutes": 1},
    ] + diagnostics("native-arm"), needs=["bootstrap", "native-arm-inputs"],
        **{"runs-on": "ubuntu-24.04-arm", "env": {"GOTOOLCHAIN": "local"}})
    windows_inputs = hosted_job("native-windows-inputs", 50, provision_steps() + [
        {"run": "sudo -n apt-get install --yes --no-install-recommends gcc-mingw-w64-x86-64"},
    ] + candidate_steps() + [
        {"run": "scripts/run_ci_required.sh --phase native-windows-inputs"},
        {**artifact_upload(f"seen-windows-iocp-inputs-{SOURCE_SHA}",
                           "target/seen-build/windows-iocp-inputs.tar.gz", compression=0),
         "timeout-minutes": 1},
    ] + diagnostics("native-windows-inputs"), needs="bootstrap")
    windows_archive = {"uses": DOWNLOAD_ARTIFACT, "with": {
        "name": f"seen-windows-iocp-inputs-{SOURCE_SHA}",
        "path": "target/seen-build/windows-iocp-transfer/",
    }}
    # Native Windows execution is bounded by a kernel job object inside the script.
    windows = {"name": "native-windows", "runs-on": "windows-2025", "timeout-minutes": 30,
               "needs": "native-windows-inputs", "defaults": {"run": {"shell": "pwsh"}}, "steps": [
        {"uses": CHECKOUT, "with": {"ref": SOURCE_SHA}},
        windows_archive,
        {"timeout-minutes": 20,
         "run": "./scripts/verify_windows_iocp.ps1 "
                "-Archive target/seen-build/windows-iocp-transfer/windows-iocp-inputs.tar.gz "
                "-Report target/seen-build/windows-iocp-qualification/report.json"},
        {**artifact_upload(f"seen-windows-iocp-qualification-{SOURCE_SHA}",
                           "target/seen-build/windows-iocp-qualification/"),
         "timeout-minutes": 1},
    ]}
    required_provision = provision_steps()
    # Diagnostics still need exact source when an upstream dependency fails.
    required_provision[1]["if"] = DIAGNOSTIC_IF
    required = hosted_job("required", 30, required_provision[:2] + [
        {"env": {"BOOTSTRAP_RESULT": "${{ needs.bootstrap.result }}",
                 "ACCEPTANCE_RESULT": "${{ needs.acceptance.result }}",
                 "NATIVE_INPUTS_RESULT": "${{ needs.native-arm-inputs.result }}",
                 "NATIVE_RESULT": "${{ needs.native-arm.result }}",
                 "WINDOWS_INPUTS_RESULT": "${{ needs.native-windows-inputs.result }}",
                 "WINDOWS_RESULT": "${{ needs.native-windows.result }}"},
         "run": '[[ "$BOOTSTRAP_RESULT" == success && "$ACCEPTANCE_RESULT" == success '
                '&& "$NATIVE_INPUTS_RESULT" == success && "$NATIVE_RESULT" == success '
                '&& "$WINDOWS_INPUTS_RESULT" == success && "$WINDOWS_RESULT" == success ]]'},
    ] + required_provision[2:] + candidate_steps() + [
        {"uses": DOWNLOAD_ARTIFACT, "with": {
            "pattern": f"seen-acceptance-receipt-*-{SOURCE_SHA}",
            "path": "target/seen-build/acceptance-receipts/", "merge-multiple": True,
        }},
        native_archive,
        {"uses": DOWNLOAD_ARTIFACT, "with": {
            "name": f"seen-linux-arm64-qualification-{SOURCE_SHA}",
            "path": "target/seen-build/linux-arm64-qualification/",
        }},
        windows_archive,
        {"uses": DOWNLOAD_ARTIFACT, "with": {
            "name": f"seen-windows-iocp-qualification-{SOURCE_SHA}",
            "path": "target/seen-build/windows-iocp-qualification/",
        }},
        {"run": "scripts/run_ci_required.sh --phase finalize"},
        {**artifact_upload(f"seen-release-toolchain-{SOURCE_SHA}",
                           "target/seen-build/release-toolchain.tar.gz", compression=0),
         "timeout-minutes": 1},
    ] + diagnostics("required"), needs=["bootstrap", "acceptance", "native-arm-inputs", "native-arm",
                                        "native-windows-inputs", "native-windows"],
        **{"if": "${{ always() }}"})
    return {
        "name": "CI",
        "on": {"push": {"branches": ["main", "release/0.23.0"]}, "workflow_dispatch": None},
        "permissions": {"contents": "read"},
        "concurrency": {
            "group": "ci-${{ github.workflow }}-${{ github.ref }}",
            "cancel-in-progress": "${{ github.ref == 'refs/heads/main' || "
                                   "contains(github.event.head_commit.message, '[replace-ci]') }}",
        },
        "jobs": {"bootstrap": bootstrap, "acceptance": acceptance,
                 "native-arm-inputs": native_inputs, "native-arm": native,
                 "native-windows-inputs": windows_inputs, "native-windows": windows,
                 "required": required},
    }


EXPECTED_WORKFLOW = ci_policy()
EXPECTED_RELEASE_WORKFLOW = """name: Release

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Exact staged version without the v prefix'
        required: true
        type: string
      draft_release_id:
        description: 'Numeric unpublished draft release ID'
        required: true
        type: string
      probe_only:
        description: 'Read-only draft access check before tag remediation'
        required: false
        default: false
        type: boolean

permissions:
  actions: read
  contents: write
  id-token: write

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

jobs:
  windows-smoke:
    name: windows-smoke
    runs-on: windows-2025
    timeout-minutes: 15
    permissions:
      # Unpublished draft metadata and assets require write-scoped access.
      contents: write
    env:
      GH_TOKEN: ${{ github.token }}
    steps:
      - name: Checkout exact release commit
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
        with:
          fetch-depth: 0
          ref: ${{ github.sha }}
      - name: Verify version-bound dispatch ref
        shell: pwsh
        env:
          RELEASE_VERSION: ${{ inputs.version }}
          PROBE_ONLY: ${{ inputs.probe_only }}
        run: |
          $policyArgs = @("--tag", "v$env:RELEASE_VERSION", "--dispatch-ref", $env:GITHUB_REF)
          if ($env:PROBE_ONLY -eq "true") { $policyArgs += "--probe-only" }
          python scripts/check_release_ci_run.py @policyArgs
          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
      - name: Verify staged Windows compiler on Windows
        if: ${{ !inputs.probe_only }}
        shell: pwsh
        env:
          RELEASE_VERSION: ${{ inputs.version }}
          SEEN_RELEASE_DRAFT_ID: ${{ inputs.draft_release_id }}
        run: ./scripts/verify_windows_release_draft.ps1 -Version $env:RELEASE_VERSION -ReleaseId $env:SEEN_RELEASE_DRAFT_ID
      - name: Prove runner-token draft access without publishing
        if: ${{ inputs.probe_only }}
        shell: pwsh
        env:
          RELEASE_VERSION: ${{ inputs.version }}
          SEEN_RELEASE_DRAFT_ID: ${{ inputs.draft_release_id }}
        run: python scripts/release_draft_api.py probe --version $env:RELEASE_VERSION --release-id $env:SEEN_RELEASE_DRAFT_ID
  release:
    name: signed-release
    needs: windows-smoke
    if: ${{ !inputs.probe_only }}
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
      SEEN_RELEASE_DRAFT_ID: ${{ inputs.draft_release_id }}
    steps:
      - name: Checkout tag
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
        with:
          fetch-depth: 0
          ref: ${{ github.sha }}
      - name: Verify dispatch is on the exact version tag
        env:
          RELEASE_VERSION: ${{ inputs.version }}
        run: |
          [[ "$RELEASE_VERSION" =~ ^[0-9]+\\.[0-9]+\\.[0-9]+$ ]]
          [[ "$GITHUB_REF_TYPE" == tag ]]
          [[ "$GITHUB_REF_NAME" == "v$RELEASE_VERSION" ]]
      - name: Verify successful authorized-source certification
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
      - name: Fetch and verify staged macOS and Windows inputs
        env:
          RELEASE_VERSION: ${{ inputs.version }}
        run: scripts/fetch_release_platform_inputs.sh "$RELEASE_VERSION"
      - name: Build sign and upload release
        env:
          RELEASE_VERSION: ${{ inputs.version }}
          SEEN_RELEASE_REQUIRE_THREE_PLATFORMS: '1'
        run: scripts/build_and_upload_release.sh "$RELEASE_VERSION"
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


def parse_workflow(raw: bytes) -> dict[str, object]:
    """Read the reviewed YAML subset, rejecting aliases, tags and duplicate keys.

    Keeping this small subset avoids an ambient YAML dependency in clean CI.
    Maps, sequences, quoted/plain scalars and literal blocks are supported;
    unsupported YAML syntax fails closed rather than being reinterpreted.
    """
    text = raw.decode("utf-8")
    if "\t" in text or text.startswith("\ufeff"):
        fail("invalid", "workflow contains tabs or a BOM")
    rows = text.splitlines()

    def scalar(value: str) -> object:
        if value.startswith('"'):
            try:
                return json.loads(value)
            except json.JSONDecodeError:
                fail("invalid", "invalid double-quoted YAML scalar")
        if value.startswith("'"):
            if not value.endswith("'"):
                fail("invalid", "unterminated quoted YAML scalar")
            return value[1:-1].replace("''", "'")
        if value.startswith("[") and value.endswith("]"):
            return [scalar(item.strip()) for item in value[1:-1].split(",") if item.strip()]
        if value == "true":
            return True
        if value == "false":
            return False
        if value.isdigit():
            return int(value)
        if value.startswith(("&", "*", "!", "{", ">", "|", "[")) or " #" in value:
            fail("invalid", "unsupported YAML scalar syntax")
        return value

    def skip(index: int) -> int:
        while index < len(rows) and (not rows[index].strip() or rows[index].lstrip().startswith("#")):
            index += 1
        return index

    def indentation(index: int) -> int:
        return len(rows[index]) - len(rows[index].lstrip(" "))

    def entry(mapping: dict, content: str, index: int, depth: int) -> int:
        key, separator, value = content.partition(":")
        if not separator or not key or any(char not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-" for char in key):
            fail("invalid", "unsupported YAML mapping key")
        if key in mapping:
            fail("invalid", f"duplicate YAML mapping key: {key}")
        value = value.strip()
        if value == "|":
            start = index
            end = index
            while end < len(rows) and (not rows[end].strip() or indentation(end) > depth):
                end += 1
            nonempty = [indentation(number) for number in range(start, end) if rows[number].strip()]
            margin = min(nonempty) if nonempty else depth + 2
            mapping[key] = "\n".join(row[margin:] if row.strip() else "" for row in rows[start:end]).rstrip("\n") + "\n"
            return end
        if value:
            mapping[key] = scalar(value)
            return index
        next_index = skip(index)
        if next_index < len(rows) and indentation(next_index) > depth:
            mapping[key], index = block(next_index, indentation(next_index))
        else:
            mapping[key] = None
        return index

    def block(index: int, depth: int) -> tuple[object, int]:
        sequence = rows[index][depth:].startswith("- ")
        result = [] if sequence else {}
        while True:
            index = skip(index)
            if index >= len(rows) or indentation(index) < depth:
                return result, index
            if indentation(index) != depth:
                fail("invalid", "inconsistent YAML indentation")
            content = rows[index][depth:]
            index += 1
            if sequence:
                if not content.startswith("- "):
                    fail("invalid", "mixed YAML collection types")
                item = content[2:]
                if ": " in item or item.endswith(":"):
                    mapping = {}
                    index = entry(mapping, item, index, depth + 2)
                    while True:
                        index = skip(index)
                        if index >= len(rows) or indentation(index) <= depth:
                            break
                        if indentation(index) != depth + 2:
                            fail("invalid", "inconsistent sequence mapping indentation")
                        index = entry(mapping, rows[index][depth + 2:], index + 1, depth + 2)
                    result.append(mapping)
                else:
                    result.append(scalar(item))
            else:
                index = entry(result, content, index, depth)

    start = skip(0)
    if start == len(rows):
        fail("invalid", "workflow is empty")
    document, end = block(start, 0)
    if skip(end) != len(rows) or not isinstance(document, dict):
        fail("invalid", "workflow must be one mapping document")
    return document


def policy_semantics(document: object, *, steps: bool = False) -> object:
    """Display labels are not authorization; all executable fields remain exact."""
    if isinstance(document, list):
        return [policy_semantics(item, steps=steps) for item in document]
    if isinstance(document, dict):
        return {key: policy_semantics(value, steps=(key == "steps"))
                for key, value in document.items() if not (steps and key == "name")}
    return document


def same_policy(actual: object, expected: object) -> bool:
    # JSON preserves scalar types: YAML false must not compare equal to integer 0.
    return json.dumps(policy_semantics(actual), sort_keys=True) == json.dumps(
        policy_semantics(expected), sort_keys=True
    )


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
    if not same_policy(parse_workflow(files[WORKFLOW]), EXPECTED_WORKFLOW):
        fail("invalid", "CI graph, containment, candidate transfer or final certification policy differs")
    if not same_policy(parse_workflow(files[RELEASE_WORKFLOW]), parse_workflow(EXPECTED_RELEASE_WORKFLOW)):
        fail("invalid", "release authorization, exact-source, signing or native Windows policy differs")
    return {
        "active_workflows": workflows,
        "checkout": "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
        "gate": "scripts/run_ci_required.sh",
        "go": "1.26.5",
        "host_tools": [
            "bubblewrap",
            "clang-20",
            "gcc-aarch64-linux-gnu",
            "g++-aarch64-linux-gnu",
            "gcc-mingw-w64-x86-64",
            "libc6-dev-arm64-cross",
            "libclang-rt-20-dev",
            "libvulkan-dev",
            "mesa-vulkan-drivers",
            "lld-20",
            "llvm-20",
            "nvidia-cuda-nvcc-cu12==12.8.93 (pinned ptxas only)",
            "ripgrep",
            "spirv-tools",
            "strace",
        ],
        "finite_tsan_host_tools_when_requested": [
            "llvm-20-dev (same pinned LLVM package version)",
            "cmake",
            "ninja-build",
            "gnupg",
            "patch",
        ],
        "host_tools_provisioning": "ubuntu-packages-pinned-assembler-scoped-apparmor",
        "setup_go": "actions/setup-go@b7ad1dad31e06c5925ef5d2fc7ad053ef454303e",
        "job_count": 6 + len(ACCEPTANCE_GROUPS),
        "job_graph": {"bootstrap": [], "acceptance": ["bootstrap"],
                      "native-arm-inputs": ["bootstrap"],
                      "native-arm": ["bootstrap", "native-arm-inputs"],
                      "native-windows-inputs": ["bootstrap"],
                      "native-windows": ["native-windows-inputs"],
                      "required": ["bootstrap", "acceptance", "native-arm-inputs", "native-arm",
                                   "native-windows-inputs", "native-windows"]},
        "acceptance_groups": list(ACCEPTANCE_GROUPS),
        "candidate": "same-run-source-bound-unverified-stage3",
        "release_artifact_producer": "required-only-after-all-five-receipts-and-native-arm-proof",
        "platforms": {
            "linux-arm64": "required-native-qualification",
            "linux-x86_64": "required",
            "macos-arm64": "release-required-input",
            "windows-x86_64": "required-native-iocp-qualification",
        },
        "required_check": "CI / required",
        "release_cosign": "v3.1.3",
        "release_ci_attestation": "scripts/verify_release_ci_run.sh",
        "release_checkout_ref": "explicit-event-commit-sha",
        "release_cpu_baseline": "x86-64",
        "release_signing": "github-oidc-keyless",
        "release_windows_runner": "windows-2025-native-smoke-required",
        "release_toolchain_artifact": "exact-authorized-source-run",
        "release_workflow": RELEASE_WORKFLOW,
        "runner": "ubuntu-24.04",
        "native_arm_runner": "ubuntu-24.04-arm",
        "native_windows_runner": "windows-2025-job-object-capped",
        "timeout_minutes": {"bootstrap": 120, "acceptance": 195,
                            "native-arm-inputs": 145, "native-arm": 50,
                            "native-windows-inputs": 50, "native-windows": 30, "required": 30},
        "critical_path_seconds": 20700,
        "hosted_workload_budget": "phase-limit-minus-monotonic-setup-minus-600s-reserve-minus-1s-rounding",
        "diagnostic_step_timeout_seconds": 60,
        "trigger": "push-main-or-release-0.23.0-or-explicit-manual",
        "upload_artifact": "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
        "version": 15,
        "download_artifact": "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093",
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
