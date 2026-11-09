#!/usr/bin/env python3
"""Mutation coverage for hosted certification and release authorization semantics."""

import copy
import importlib.util
import json
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


policy = load("workflow_policy", "scripts/check_ci_workflows.py")
containment = load("containment_policy", "scripts/check_ci_containment.py")


def yaml_document(value, depth=0):
    """Emit ordinary YAML with JSON-quoted scalars; no production serializer reuse."""
    rows = []
    if isinstance(value, dict):
        for key, item in value.items():
            prefix = " " * depth + key + ":"
            if isinstance(item, (dict, list)):
                rows.append(prefix)
                rows.extend(yaml_document(item, depth + 2))
            else:
                rows.append(prefix + (" " + json.dumps(item) if item is not None else ""))
    else:
        for item in value:
            if isinstance(item, dict):
                nested = yaml_document(item, depth + 2)
                rows.append(" " * depth + "- " + nested[0][depth + 2:])
                rows.extend(nested[1:])
            else:
                rows.append(" " * depth + "- " + json.dumps(item))
    return rows


# The native Windows job has no Linux scope, uptime clock or Go toolchain; its
# kernel job object caps are asserted by the script and the finalized report.
NATIVE_WINDOWS = "native-windows"


class WorkflowPolicy(unittest.TestCase):
    def setUp(self):
        self.ci = policy.parse_workflow((ROOT / policy.WORKFLOW).read_bytes())
        self.linux_jobs = [name for name in self.ci["jobs"] if name != NATIVE_WINDOWS]
        self.release = policy.parse_workflow((ROOT / policy.RELEASE_WORKFLOW).read_bytes())

    def validate(self, ci=None, release=None):
        cache = ROOT / ".seen/workflow-policy-unit-tests"
        cache.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=cache) as directory:
            root = Path(directory)
            workflows = root / ".github/workflows"
            workflows.mkdir(parents=True)
            for name, value in (("ci.yml", self.ci if ci is None else ci),
                                ("release.yml", self.release if release is None else release)):
                (workflows / name).write_text("\n".join(yaml_document(value)) + "\n")
            return policy.validate(root, 256, 4 * 1024 * 1024, 0)

    def test_semantic_formatting_and_display_labels_are_not_authorization(self):
        for job in self.ci["jobs"].values():
            for step in job["steps"]:
                step["name"] = "Display label is freely editable"
        report = self.validate()
        self.assertEqual(report["critical_path_seconds"], 20700)

    def test_graph_cannot_skip_fail_or_omit_acceptance(self):
        mutations = [
            lambda ci: ci["jobs"]["acceptance"]["strategy"].update({"fail-fast": True}),
            lambda ci: ci["jobs"]["acceptance"]["strategy"].update({"fail-fast": 0}),
            lambda ci: ci["jobs"]["acceptance"].update({"if": "${{ false }}"}),
            lambda ci: ci["jobs"]["acceptance"].update({"continue-on-error": True}),
            lambda ci: ci["jobs"]["required"].update({"needs": "bootstrap"}),
            lambda ci: ci["jobs"]["required"].pop("if"),
            lambda ci: ci["jobs"]["required"]["steps"].remove(next(
                step for step in ci["jobs"]["required"]["steps"]
                if "BOOTSTRAP_RESULT" in step.get("env", {}))),
            lambda ci: next(step for step in ci["jobs"]["required"]["steps"]
                            if "BOOTSTRAP_RESULT" in step.get("env", {})).update({"run": "true"}),
            lambda ci: ci["jobs"].update({"unreviewed": copy.deepcopy(ci["jobs"]["bootstrap"])}),
        ]
        for mutate in mutations:
            ci = copy.deepcopy(self.ci)
            mutate(ci)
            with self.subTest(mutate=mutate), self.assertRaises(policy.WorkflowError):
                self.validate(ci=ci)

    def test_required_groups_cannot_be_omitted_or_replaced_by_combined_group(self):
        for omitted in (("gpu-runtime",), ("async",), ("gpu-runtime", "async"),
                        ("core", "io-simd", "gpu-emitter", "gpu-runtime", "async")):
            for combined in (False, True):
                ci = copy.deepcopy(self.ci)
                groups = ci["jobs"]["acceptance"]["strategy"]["matrix"]["group"]
                groups[:] = [group for group in groups if group not in omitted]
                if combined:
                    groups.append("gpu-runtime-async")
                with self.subTest(omitted=omitted, combined=combined):
                    with self.assertRaises(policy.WorkflowError):
                        self.validate(ci=ci)

    def test_finite_sdk_provisioning_cannot_drop_a_consumer_or_add_a_nonconsumer(self):
        for groups in (("core",), ("async",),
                       ("core", "async", "gpu-runtime"),
                       ("core", "async", "gpu-emitter")):
            ci = copy.deepcopy(self.ci)
            provision = next(step for step in ci["jobs"]["acceptance"]["steps"]
                             if step.get("run") == "scripts/provision_ci_host.sh")
            condition = " || ".join(f"matrix.group == '{group}'" for group in groups)
            provision["env"]["SEEN_PREPARE_FINITE_TSAN_SDK"] = (
                "${{ (" + condition + ") && '1' || '0' }}"
            )
            with self.subTest(groups=groups), self.assertRaises(policy.WorkflowError):
                self.validate(ci=ci)

    def test_dependency_failure_cannot_prevent_source_checkout_for_diagnostics(self):
        ci = copy.deepcopy(self.ci)
        steps = ci["jobs"]["required"]["steps"]
        gate = next(step for step in steps if "BOOTSTRAP_RESULT" in step.get("env", {}))
        steps.remove(gate)
        steps.insert(0, gate)
        with self.assertRaises(policy.WorkflowError):
            self.validate(ci=ci)

    def test_every_phase_preserves_scopes_toolchain_and_deadlines(self):
        for name in self.linux_jobs:
            for key, value in (("timeout-minutes", 360), ("runs-on", "ubuntu-latest"),
                               ("permissions", {"contents": "write"})):
                ci = copy.deepcopy(self.ci)
                ci["jobs"][name][key] = value
                with self.subTest(job=name, key=key), self.assertRaises(policy.WorkflowError):
                    self.validate(ci=ci)
            ci = copy.deepcopy(self.ci)
            steps = ci["jobs"][name]["steps"]
            steps[:] = [step for step in steps if step.get("run") not in {
                "scripts/provision_ci_host.sh", "scripts/provision_ci_linux_arm64_host.sh"}]
            with self.assertRaises(policy.WorkflowError):
                self.validate(ci=ci)
            ci = copy.deepcopy(self.ci)
            go_step = next(step for step in ci["jobs"][name]["steps"]
                           if step.get("uses", "").startswith("actions/setup-go@"))
            go_step["with"]["go-version"] = "1.26"
            with self.subTest(job=name, changed="Go version"), self.assertRaises(policy.WorkflowError):
                self.validate(ci=ci)

    def test_hosted_clock_and_bounded_diagnostics_cannot_be_removed_or_delayed(self):
        for name in self.linux_jobs:
            steps = self.ci["jobs"][name]["steps"]
            self.assertEqual(steps[0]["if"], "${{ !cancelled() }}")
            for mutation in ("remove-clock", "late-clock", "unbound-diagnostic",
                             "cancelled-diagnostic", "unbound-publication"):
                ci = copy.deepcopy(self.ci)
                changed = ci["jobs"][name]["steps"]
                if mutation == "remove-clock":
                    changed.pop(0)
                elif mutation == "late-clock":
                    changed.insert(2, changed.pop(0))
                elif mutation in ("unbound-diagnostic", "cancelled-diagnostic"):
                    diagnostic = next(step for step in changed
                                      if step.get("run") == "python3 scripts/collect_verification_logs.py")
                    if mutation == "unbound-diagnostic":
                        diagnostic.pop("timeout-minutes")
                    else:
                        diagnostic["if"] = "${{ always() }}"
                else:
                    publication = next(step for step in changed
                                       if step.get("uses", "").startswith("actions/upload-artifact@")
                                       and "if" not in step)
                    publication.pop("timeout-minutes")
                with self.subTest(job=name, mutation=mutation), self.assertRaises(policy.WorkflowError):
                    self.validate(ci=ci)

    def test_every_diagnostic_and_post_gate_publication_has_a_one_minute_bound(self):
        for name in self.linux_jobs:
            steps = self.ci["jobs"][name]["steps"]
            gate_index = next(index for index, step in enumerate(steps)
                              if step.get("run", "").startswith("scripts/run_ci_required.sh"))
            diagnostics = [step for step in steps[gate_index + 1:]
                           if step.get("if") == "${{ !cancelled() }}"]
            self.assertEqual(len(diagnostics), 4)
            self.assertTrue(all(step["timeout-minutes"] == 1 for step in steps[gate_index + 1:]))
            success_publications = [step for step in steps[gate_index + 1:] if "if" not in step]
            self.assertLessEqual(len(success_publications), 2)
            # Default success gating must protect receipts, candidates and certification.
            self.assertTrue(all("continue-on-error" not in step for step in success_publications))

    def test_candidate_cannot_be_release_eligible_or_cross_run(self):
        for job_name in ("bootstrap", "acceptance", "required"):
            ci = copy.deepcopy(self.ci)
            for step in ci["jobs"][job_name]["steps"]:
                if step.get("uses", "").startswith("actions/upload-artifact@") and "candidate" in step["with"]["name"]:
                    step["with"]["name"] = "seen-release-toolchain-${{ github.sha }}"
                    break
                if step.get("uses", "").startswith("actions/download-artifact@"):
                    step["with"]["run-id"] = "12345"
                    break
            with self.subTest(job=job_name), self.assertRaises(policy.WorkflowError):
                self.validate(ci=ci)
        ci = copy.deepcopy(self.ci)
        ci["jobs"]["acceptance"]["steps"].append({"run": "scripts/prepare_release_toolchain_artifact.sh"})
        with self.assertRaises(policy.WorkflowError):
            self.validate(ci=ci)

    def test_native_windows_execution_cannot_be_skipped_moved_or_unbounded(self):
        def verify_step(doc):
            return next(step for step in doc["jobs"][NATIVE_WINDOWS]["steps"]
                        if "verify_windows_iocp.ps1" in step.get("run", ""))
        mutations = {
            "linux-runner": lambda doc: doc["jobs"][NATIVE_WINDOWS].update({"runs-on": "ubuntu-24.04"}),
            "unpinned-runner": lambda doc: doc["jobs"][NATIVE_WINDOWS].update({"runs-on": "windows-latest"}),
            "deadline": lambda doc: doc["jobs"][NATIVE_WINDOWS].update({"timeout-minutes": 360}),
            "skip-run": lambda doc: doc["jobs"][NATIVE_WINDOWS]["steps"].remove(verify_step(doc)),
            "tolerate": lambda doc: verify_step(doc).update({"continue-on-error": True}),
            "unbounded-step": lambda doc: verify_step(doc).pop("timeout-minutes"),
            "other-script": lambda doc: verify_step(doc).update(
                {"run": verify_step(doc)["run"].replace("verify_windows_iocp", "verify_windows_release_draft")}),
            "optional-dependency": lambda doc: doc["jobs"]["required"]["needs"].remove(NATIVE_WINDOWS),
            "ignored-result": lambda doc: next(
                step for step in doc["jobs"]["required"]["steps"]
                if "WINDOWS_RESULT" in step.get("env", {}))["env"].pop("WINDOWS_RESULT"),
            "unverified-report": lambda doc: doc["jobs"]["required"]["steps"].remove(next(
                step for step in doc["jobs"]["required"]["steps"]
                if step.get("with", {}).get("name", "").startswith("seen-windows-iocp-qualification-"))),
        }
        for name, mutate in mutations.items():
            ci = copy.deepcopy(self.ci)
            mutate(ci)
            with self.subTest(mutation=name), self.assertRaises(policy.WorkflowError):
                self.validate(ci=ci)

    def test_failed_evidence_never_uploads_bulk_ir(self):
        ci = copy.deepcopy(self.ci)
        for step in ci["jobs"]["acceptance"]["steps"]:
            if step.get("uses", "").startswith("actions/upload-artifact@"):
                step["with"]["path"] = "target/**/*.ll"
                break
        with self.assertRaises(policy.WorkflowError):
            self.validate(ci=ci)

    def test_release_source_signing_and_native_windows_are_still_required(self):
        for mutate in (
            lambda doc: doc["jobs"]["release"].pop("needs"),
            lambda doc: doc["jobs"]["windows-smoke"].update({"runs-on": "ubuntu-24.04"}),
            lambda doc: doc["jobs"]["release"]["env"].update({"SEEN_RELEASE_SIGN_MODE": "unsigned"}),
            lambda doc: doc["jobs"]["release"]["steps"][0]["with"].pop("ref"),
            lambda doc: doc["permissions"].update({"actions": "write"}),
        ):
            release = copy.deepcopy(self.release)
            mutate(release)
            with self.subTest(mutate=mutate), self.assertRaises(policy.WorkflowError):
                self.validate(release=release)

    def test_duplicate_alias_and_unsupported_yaml_fail_closed(self):
        for raw in (b"name: CI\nname: Release\n", b"jobs: &alias\n", b"jobs:\n  required: *alias\n"):
            with self.assertRaises(policy.WorkflowError):
                policy.parse_workflow(raw)

    def test_containment_preserves_local_full_and_staged_hosted_contract(self):
        valid = json.loads((ROOT / "docs/architecture/ci-containment.json").read_text())
        self.assertEqual(containment.validate(valid)["execution"]["default_local"], "full")
        for section, key, value in (("memory", "swap_max_bytes", 1),
                                    ("memory", "swap_max_bytes", False),
                                    ("tasks", "hard_max", 25),
                                    ("execution", "critical_path_seconds", 21600),
                                    ("execution", "default_local", "bootstrap"),
                                    ("execution", "acceptance_fail_fast", True)):
            document = copy.deepcopy(valid)
            document[section][key] = value
            with self.subTest(section=section, key=key), self.assertRaises(containment.ContractError):
                containment.validate(document)

    def test_containment_cannot_authorize_incomplete_or_obsolete_acceptance(self):
        valid = json.loads((ROOT / "docs/architecture/ci-containment.json").read_text())
        for omitted in (("gpu-runtime",), ("async",), ("gpu-runtime", "async"),
                        tuple(valid["execution"]["acceptance_groups"])):
            for combined in (False, True):
                document = copy.deepcopy(valid)
                groups = document["execution"]["acceptance_groups"]
                groups[:] = [group for group in groups if group not in omitted]
                if combined:
                    groups.append("gpu-runtime-async")
                with self.subTest(omitted=omitted, combined=combined):
                    with self.assertRaises(containment.ContractError):
                        containment.validate(document)

    def test_hosted_budget_reserve_cannot_disappear_or_grow_the_outer_deadline(self):
        valid = json.loads((ROOT / "docs/architecture/ci-containment.json").read_text())
        for key, value in (("reserve_seconds", 0), ("subtract_elapsed_setup", False),
                           ("rounding_seconds", 0), ("diagnostic_step_seconds", 120)):
            document = copy.deepcopy(valid)
            document["execution"]["hosted_workload_budget"][key] = value
            with self.subTest(key=key), self.assertRaises(containment.ContractError):
                containment.validate(document)
        for phase, limit in valid["execution"]["job_timeout_seconds"].items():
            document = copy.deepcopy(valid)
            document["execution"]["job_timeout_seconds"][phase] = limit + 1
            with self.subTest(phase=phase), self.assertRaises(containment.ContractError):
                containment.validate(document)


class HostedBudget(unittest.TestCase):
    """Exercise real runner preflight, without starting a build-capable scope."""

    def setUp(self):
        cache = ROOT / ".seen/workflow-policy-unit-tests"
        cache.mkdir(parents=True, exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        # Stop after budget selection and before artifact/scope initialization.
        uname = self.root / "uname"
        uname.write_text("#!/bin/sh\nprintf 'preflight-only\\n'\n")
        uname.chmod(0o700)

    @staticmethod
    def now():
        return int(Path("/proc/uptime").read_text().split(".")[0])

    def preflight(self, phase, start=None, hosted=True):
        environment = dict(os.environ, GITHUB_ACTIONS="true" if hosted else "false",
                           PATH=str(self.root) + os.pathsep + os.defpath)
        environment.pop("SEEN_CI_JOB_STARTED_UPTIME_SECS", None)
        if start is not None:
            environment["SEEN_CI_JOB_STARTED_UPTIME_SECS"] = str(start)
        command = ["bash", str(ROOT / "scripts/run_ci_required.sh"), "--phase", phase]
        if phase == "acceptance":
            command += ["--group", "gpu-runtime"]
        return subprocess.run(command, env=environment, capture_output=True, text=True, timeout=5)

    def test_first_step_records_monotonic_clock_without_checkout(self):
        destination = self.root / "github-env"
        environment = dict(os.environ, GITHUB_ENV=str(destination))
        before = self.now()
        ci = policy.parse_workflow((ROOT / policy.WORKFLOW).read_bytes())
        command = ci["jobs"]["acceptance"]["steps"][0]["run"]
        result = subprocess.run(["bash", "-euo", "pipefail", "-c", command],
                                cwd=self.root, env=environment, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        name, value = destination.read_text().strip().split("=")
        self.assertEqual(name, "SEEN_CI_JOB_STARTED_UPTIME_SECS")
        self.assertLessEqual(before, int(value))
        self.assertLessEqual(int(value), self.now())

    def test_all_hosted_phases_subtract_setup_reserve_and_rounding(self):
        if self.now() < 192:
            self.skipTest("requires at least 192s host uptime")
        for phase, limit in (("bootstrap", 7200), ("acceptance", 11700),
                             ("native-arm-inputs", 8700), ("native-arm", 3000),
                             ("native-windows-inputs", 3000), ("finalize", 1800)):
            result = self.preflight(phase, self.now() - 192)
            self.assertEqual(result.returncode, 126, result.stderr)
            budget = re.search(r"setup=(\d+)s workload=(\d+)s reserve=(\d+)s rounding=(\d+)s",
                               result.stderr)
            self.assertIsNotNone(budget, result.stderr)
            setup, workload, reserve, rounding = map(int, budget.groups())
            self.assertGreaterEqual(setup, 192)
            self.assertEqual((reserve, rounding), (600, 1))
            self.assertEqual(setup + workload + reserve + rounding, limit)
            self.assertIn("core.001b.invalid", result.stderr)

    def test_setup_exhaustion_is_timeout_before_owned_work_starts(self):
        if self.now() < 1200:
            self.skipTest("requires enough uptime to exhaust the finalize budget")
        result = self.preflight("finalize", max(0, self.now() - 1800))
        self.assertEqual(result.returncode, 124, result.stderr)
        self.assertIn("core.001b.timeout", result.stderr)

    def test_missing_malformed_future_clock_and_hosted_full_fail_closed(self):
        for start in (None, "", "-1", "not-a-clock", self.now() + 1000, "99999999999"):
            result = self.preflight("acceptance", start)
            with self.subTest(start=start):
                self.assertEqual(result.returncode, 126, result.stderr)
                self.assertNotIn("hosted phase=", result.stderr)
        result = self.preflight("all", self.now())
        self.assertEqual(result.returncode, 126, result.stderr)

    def test_local_full_and_named_phases_do_not_consume_hosted_reserve(self):
        for phase in ("all", "bootstrap", "acceptance", "native-arm-inputs", "native-arm",
                      "native-windows-inputs", "finalize"):
            result = self.preflight(phase, hosted=False)
            self.assertEqual(result.returncode, 126, result.stderr)
            self.assertNotIn("hosted phase=", result.stderr)
            self.assertIn("core.001b.invalid", result.stderr)



if __name__ == "__main__":
    unittest.main()
