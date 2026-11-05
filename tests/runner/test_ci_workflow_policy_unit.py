#!/usr/bin/env python3
"""Mutation coverage for hosted certification and release authorization semantics."""

import copy
import importlib.util
import json
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


class WorkflowPolicy(unittest.TestCase):
    def setUp(self):
        self.ci = policy.parse_workflow((ROOT / policy.WORKFLOW).read_bytes())
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
        self.assertEqual(report["job_count"], 6)
        self.assertEqual(report["critical_path_seconds"], 20700)
        self.assertEqual(set(report["acceptance_groups"]),
                         {"core", "io-simd", "gpu-emitter", "gpu-runtime-async"})

    def test_graph_cannot_skip_fail_or_omit_acceptance(self):
        mutations = [
            lambda ci: ci["jobs"]["acceptance"]["strategy"]["matrix"]["group"].pop(),
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

    def test_dependency_failure_cannot_prevent_source_checkout_for_diagnostics(self):
        ci = copy.deepcopy(self.ci)
        steps = ci["jobs"]["required"]["steps"]
        gate = next(step for step in steps if "BOOTSTRAP_RESULT" in step.get("env", {}))
        steps.remove(gate)
        steps.insert(0, gate)
        with self.assertRaises(policy.WorkflowError):
            self.validate(ci=ci)

    def test_every_phase_preserves_scopes_toolchain_and_deadlines(self):
        for name in self.ci["jobs"]:
            for key, value in (("timeout-minutes", 360), ("runs-on", "ubuntu-latest"),
                               ("permissions", {"contents": "write"})):
                ci = copy.deepcopy(self.ci)
                ci["jobs"][name][key] = value
                with self.subTest(job=name, key=key), self.assertRaises(policy.WorkflowError):
                    self.validate(ci=ci)
            ci = copy.deepcopy(self.ci)
            steps = ci["jobs"][name]["steps"]
            steps[:] = [step for step in steps if step.get("run") != "scripts/provision_ci_host.sh"]
            with self.assertRaises(policy.WorkflowError):
                self.validate(ci=ci)
            ci = copy.deepcopy(self.ci)
            go_step = next(step for step in ci["jobs"][name]["steps"]
                           if step.get("uses", "").startswith("actions/setup-go@"))
            go_step["with"]["go-version"] = "1.26"
            with self.subTest(job=name, changed="Go version"), self.assertRaises(policy.WorkflowError):
                self.validate(ci=ci)

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


if __name__ == "__main__":
    unittest.main()
