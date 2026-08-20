#!/usr/bin/env python3
"""Unit coverage for the CORE-002A compatibility-manifest validator."""

from __future__ import annotations

import copy
import importlib.util
import io
import json
import os
import unittest
from unittest import mock
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_compatibility_manifest",
    ROOT / "scripts" / "check_compatibility_manifest.py",
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("could not load compatibility-manifest checker")
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
HAPPY = json.loads(
    (ROOT / "tests/fixtures/core-002a/happy/compatibility-manifest.json").read_text()
)


class CompatibilityManifestTests(unittest.TestCase):
    def changed(self, *path: str, value: object) -> dict[str, object]:
        document = copy.deepcopy(HAPPY)
        target: object = document
        for key in path[:-1]:
            if isinstance(target, dict):
                target = target[key]
            elif isinstance(target, list):
                target = target[int(key)]
            else:
                self.fail(f"invalid test path: {path}")
        if isinstance(target, dict):
            target[path[-1]] = value
        elif isinstance(target, list):
            target[int(path[-1])] = value
        else:
            self.fail(f"invalid test path: {path}")
        return document

    def assert_code(self, document: object, code: str) -> None:
        with self.assertRaisesRegex(CHECKER.ContractError, rf"core\.002a\.{code}:"):
            CHECKER.validate(document)

    def test_happy(self) -> None:
        self.assertEqual(CHECKER.validate(copy.deepcopy(HAPPY)), HAPPY)

    def test_top_level_contract(self) -> None:
        self.assert_code([], "invalid")
        unknown = copy.deepcopy(HAPPY)
        unknown["unknown"] = True
        self.assert_code(unknown, "invalid")
        self.assert_code(self.changed("schema", value="other"), "invalid")
        self.assert_code(self.changed("schema_version", value=2), "invalid")
        self.assert_code(self.changed("release_version", value="v1"), "invalid")

    def test_compiler_contract(self) -> None:
        self.assert_code(self.changed("components", value={}), "invalid")
        self.assert_code(self.changed("components", "compiler", value={}), "invalid")
        self.assert_code(
            self.changed("components", "compiler", "version", value="0.10.2"),
            "invalid",
        )
        self.assert_code(
            self.changed("components", "compiler", "layout_abi", value="bad value"),
            "invalid",
        )

    def test_llvm_and_package_client_contract(self) -> None:
        self.assert_code(self.changed("components", "llvm", value={}), "invalid")
        for value in (True, 17, 256):
            self.assert_code(
                self.changed("components", "llvm", "minimum_major", value=value),
                "invalid",
            )
        self.assert_code(self.changed("components", "package_client", value={}), "invalid")
        self.assert_code(
            self.changed("components", "package_client", "version", value="0.10.2"),
            "invalid",
        )
        self.assert_code(
            self.changed("components", "package_client", "protocol", value="bad value"),
            "invalid",
        )

    def test_runtime_and_stdlib_contract(self) -> None:
        self.assert_code(self.changed("components", "runtime", value={}), "invalid")
        self.assert_code(
            self.changed("components", "runtime", "abi", value="bad value"),
            "invalid",
        )
        self.assert_code(self.changed("components", "standard_library", value={}), "invalid")
        self.assert_code(
            self.changed("components", "standard_library", "version", value="next"),
            "invalid",
        )
        for value in (False, 0, 256):
            self.assert_code(
                self.changed(
                    "components", "standard_library", "module_manifest_version", value=value
                ),
                "invalid",
            )

    def test_platform_contract(self) -> None:
        self.assert_code(self.changed("platforms", value={}), "invalid")
        self.assert_code(
            self.changed("platforms", "windows", value="required"), "invalid"
        )

    def test_target_limits_and_shape(self) -> None:
        self.assert_code(self.changed("targets", value={}), "limit")
        self.assert_code(self.changed("targets", value=[]), "limit")
        self.assert_code(self.changed("targets", value=[HAPPY["targets"][0]] * 33), "limit")
        self.assert_code(self.changed("targets", "0", value={}), "invalid")

    def test_target_identity_and_support(self) -> None:
        self.assert_code(self.changed("targets", "0", "name", value="plan9-amd64"), "platform")
        duplicate = copy.deepcopy(HAPPY)
        duplicate["targets"].append(copy.deepcopy(duplicate["targets"][0]))
        self.assert_code(duplicate, "invalid")
        self.assert_code(self.changed("targets", "0", "support", value="maybe"), "invalid")
        self.assert_code(self.changed("targets", "0", "triple", value="bad triple"), "invalid")

    def test_required_and_ordering_contract(self) -> None:
        wrong_required = self.changed("targets", "0", "name", value="linux-arm64")
        wrong_required["targets"][0]["triple"] = "aarch64-unknown-linux-gnu"
        self.assert_code(wrong_required, "platform")
        missing_required = self.changed(
            "targets", "0", "support", value="declared-toolchain-dependent"
        )
        self.assert_code(missing_required, "platform")
        unordered = copy.deepcopy(HAPPY)
        unordered["targets"].insert(
            0,
            {
                "name": "macos-arm64",
                "support": "declared-toolchain-dependent",
                "triple": "arm64-apple-darwin",
            },
        )
        self.assert_code(unordered, "invalid")

    def test_byte_parser(self) -> None:
        raw = json.dumps(HAPPY).encode()
        self.assertEqual(CHECKER.parse_and_validate(raw, len(raw)), HAPPY)
        with self.assertRaisesRegex(CHECKER.ContractError, "core.002a.limit"):
            CHECKER.parse_and_validate(raw, 1)
        with self.assertRaisesRegex(CHECKER.ContractError, "core.002a.invalid"):
            CHECKER.parse_and_validate(b"\xef\xbb\xbf{}", 64)
        with self.assertRaises(UnicodeDecodeError):
            CHECKER.parse_and_validate(b"\xff", 64)
        with self.assertRaises(json.JSONDecodeError):
            CHECKER.parse_and_validate(b"{", 64)

    def test_argument_bounds_and_fuzz(self) -> None:
        self.assertEqual(CHECKER.positive_bounded("1", 2, "value"), 1)
        with self.assertRaises(CHECKER.argparse.ArgumentTypeError):
            CHECKER.positive_bounded("no", 2, "value")
        with self.assertRaises(CHECKER.argparse.ArgumentTypeError):
            CHECKER.positive_bounded("3", 2, "value")
        CHECKER.fuzz(json.dumps(HAPPY).encode(), 0.01, 1101, 64 * 1024)

    def run_main(self, *arguments: str, environment: dict[str, str] | None = None) -> int:
        class Output:
            def __init__(self) -> None:
                self.buffer = io.BytesIO()
                self.text = io.StringIO()

            def write(self, value: str) -> int:
                return self.text.write(value)

            def flush(self) -> None:
                return None

        stdout = Output()
        stderr = Output()
        active_environment = os.environ.copy()
        if environment is not None:
            active_environment.update(environment)
        with mock.patch.object(CHECKER.sys, "argv", ["checker", *arguments]), mock.patch.object(
            CHECKER.sys, "stdout", stdout
        ), mock.patch.object(CHECKER.sys, "stderr", stderr), mock.patch.dict(
            os.environ, active_environment, clear=True
        ):
            return CHECKER.main()

    def test_main_success_and_fuzz(self) -> None:
        happy = str(ROOT / "tests/fixtures/core-002a/happy/compatibility-manifest.json")
        self.assertEqual(self.run_main(happy), 0)
        self.assertEqual(self.run_main(happy, "--fuzz-seconds", "0.01", "--seed", "1101"), 0)

    def test_main_failures_and_cancellation(self) -> None:
        happy = str(ROOT / "tests/fixtures/core-002a/happy/compatibility-manifest.json")
        self.assertEqual(self.run_main(happy, "--fuzz-seconds", "-1"), 1)
        self.assertEqual(self.run_main(happy, "--max-bytes", "1"), 1)
        self.assertEqual(self.run_main(str(ROOT / "missing-core-002a.json")), 1)
        self.assertEqual(self.run_main(str(ROOT / "tests/fixtures/core-002a")), 1)
        self.assertEqual(self.run_main(happy, "--test-cancel-after-read"), 1)
        self.assertEqual(
            self.run_main(
                happy,
                "--test-cancel-after-read",
                environment={"SEEN_CORE_002A_TEST_HOOKS": "1"},
            ),
            130,
        )
        with self.assertRaises(KeyboardInterrupt):
            CHECKER.cancel(15, None)


if __name__ == "__main__":
    unittest.main()
