#!/usr/bin/env python3
"""Branch matrix for CORE-REL-002 evidence validation."""

from __future__ import annotations

import importlib.util
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_build_instrumentation", ROOT / "scripts/check_build_instrumentation.py")
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def document() -> dict[str, object]:
    return {
        "components": {"abi_shims": "compile-only",
                       "compiler_host": "source-only",
                       "native_runtime": "compile-only",
                       "seen_modules": "compile-only"},
        "modes": {"coverage": True, "debug": True,
                  "sanitizer": "undefined"},
        "schema": "seen-build-instrumentation-evidence-v1",
        "target": "linux-x86_64",
    }


class BuildInstrumentationTests(unittest.TestCase):
    def raw(self, value: object | None = None) -> bytes:
        return json.dumps(document() if value is None else value).encode()

    def assert_code(self, code: str, raw: bytes, **kwargs: object) -> None:
        with self.assertRaises(CHECKER.InstrumentationError) as raised:
            CHECKER.validate(raw, **kwargs)
        self.assertEqual(code, raised.exception.code)

    def test_happy_is_canonical(self) -> None:
        self.assertEqual("compile-only",
                         CHECKER.validate(self.raw())["components"]["abi_shims"])

    def test_invalid_limit_cancel_and_platform(self) -> None:
        value = document(); value["extra"] = True
        self.assert_code("invalid", self.raw(value))
        value = document(); value["target"] = "macos-arm64"
        self.assert_code("platform", self.raw(value))
        value = document(); value["components"]["abi_shims"] = "hardware-executed"
        self.assert_code("invalid", self.raw(value))
        self.assert_code("limit", self.raw(), max_bytes=1)
        self.assert_code("cancelled", self.raw(), cancelled=True)

    def test_duplicates_and_fuzz(self) -> None:
        self.assert_code("invalid", b'{"schema":1,"schema":2}')
        CHECKER.fuzz(self.raw(), 0.02, 1101)


if __name__ == "__main__":
    unittest.main()
