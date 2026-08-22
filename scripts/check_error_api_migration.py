#!/usr/bin/env python3
"""Validate and audit the ERR-001C structured-error API migration."""

from __future__ import annotations

import argparse
import json
import random
import re
import sys
import time
from pathlib import Path

SCHEMA = "seen-error-api-migration-v1"
MAX_BYTES = 1_048_576
MAX_MODULES = 128
FIELDS = {"bootstrap_exceptions", "modules", "schema"}
RESULT_STRING = re.compile(r"Result\s*<[^\n>]*(?:>|,)[^\n>]*\bString\s*>")
LEGACY_RESULT = re.compile(r"\bFsFileResult\b")
EXCEPTION_SIGNATURES = {
    "io.file.readText": "fun readText(path: String) r: String",
    "io.file.writeText": "fun writeText(path: String, content: String) r: Bool",
    "io.file.writeTextAtomically": "fun writeTextAtomically(path: String, content: String) r: Bool",
    "io.file.appendText": "fun appendText(path: String, content: String) r: Bool",
    "io.file.exists": "fun exists(path: String) r: Bool",
    "io.file.deleteFile": "fun deleteFile(path: String) r: Bool",
    "io.file.createDirectory": "fun createDirectory(path: String) r: Bool",
    "io.file.size": "fun size(path: String) r: Int",
}


class ContractError(ValueError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise ContractError(code, message)


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            fail("invalid", f"duplicate field: {key}")
        value[key] = item
    return value


def valid_path(value: object) -> bool:
    return (
        isinstance(value, str)
        and 0 < len(value) <= 256
        and not value.startswith("/")
        and ".." not in Path(value).parts
        and value.endswith(".seen")
    )


def validate(raw: bytes, max_bytes: int = MAX_BYTES, cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "migration contract byte limit exceeded")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {error}")
    if not isinstance(value, dict) or set(value) != FIELDS or value.get("schema") != SCHEMA:
        fail("invalid", "invalid migration contract")
    if cancelled:
        fail("cancelled", "migration audit is cancelled")
    modules = value["modules"]
    exceptions = value["bootstrap_exceptions"]
    if (
        not isinstance(modules, list)
        or not 1 <= len(modules) <= MAX_MODULES
        or len(modules) != len(set(modules))
        or modules != sorted(modules)
        or not all(valid_path(module) for module in modules)
    ):
        fail("invalid", "modules must be a bounded sorted unique path list")
    if (
        not isinstance(exceptions, list)
        or exceptions != sorted(EXCEPTION_SIGNATURES)
    ):
        fail("invalid", "bootstrap exceptions do not match the frozen inventory")
    return value


def audit(value: dict[str, object], root: Path) -> list[str]:
    violations: list[str] = []
    for relative in value["modules"]:
        path = root / relative
        try:
            source = path.read_text()
        except OSError as error:
            violations.append(f"{relative}: unavailable: {error}")
            continue
        for line_number, line in enumerate(source.splitlines(), 1):
            if RESULT_STRING.search(line):
                violations.append(f"{relative}:{line_number}: string-error Result")
            if LEGACY_RESULT.search(line):
                violations.append(f"{relative}:{line_number}: legacy FsFileResult")
    file_source = (root / "seen_std/src/io/file.seen").read_text()
    if "Bootstrap-safe" not in file_source:
        violations.append("seen_std/src/io/file.seen: missing bootstrap boundary documentation")
    for name, signature in EXCEPTION_SIGNATURES.items():
        if signature not in file_source:
            violations.append(f"{name}: frozen bootstrap signature changed")
    return sorted(violations)


def canonical_bytes(value: dict[str, object]) -> bytes:
    return (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode()


def fuzz(raw: bytes, seconds: float, seed: int) -> tuple[int, int]:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = rejected = 0
    while time.monotonic() < deadline:
        changed = bytearray(raw)
        if changed:
            changed[rng.randrange(len(changed))] = rng.randrange(256)
        cases += 1
        try:
            validate(bytes(changed))
        except ContractError:
            rejected += 1
    return cases, rejected


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--validate", required=True, type=Path)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--max-bytes", default=MAX_BYTES, type=int)
    parser.add_argument("--fuzz-seconds", default=0.0, type=float)
    parser.add_argument("--seed", default=1101, type=int)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        raw = args.validate.read_bytes()
        value = validate(raw, args.max_bytes, args.test_cancel_after_read)
        if args.root:
            violations = audit(value, args.root.resolve())
            if violations:
                fail("invalid", "; ".join(violations))
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical_bytes(value))
    except ContractError as error:
        print(f"err.001c.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"err.001c.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
