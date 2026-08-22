#!/usr/bin/env python3
"""Discover and validate canonical TEST-001A test manifests."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-test-discovery-v1"
MAX_BYTES = 16 * 1024 * 1024
MAX_PATH_BYTES = 4096
MAX_TESTS = 100_000
FIELDS = {"schema", "tests"}
TEST_FIELDS = {"category", "ignored", "path", "platform", "privileged", "slow"}
ROOTS = ("compiler_seen/tests", "seen_std/tests",
         "tests/fixtures/external_package/tests", "tests/misc_root_tests")


class TestDiscoveryError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise TestDiscoveryError(code, message)


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail("invalid", f"duplicate field: {key}")
        result[key] = value
    return result


def canonical_path(path: object) -> bool:
    return (
        isinstance(path, str)
        and bool(path)
        and len(path.encode()) <= MAX_PATH_BYTES
        and path.isascii()
        and not path.startswith("/")
        and not path.endswith("/")
        and ".." not in path
        and "//" not in path
        and "\\" not in path
        and all(character.isalnum() or character in "-._/" for character in path)
    )


def expected_category(path: str) -> str:
    if path.startswith(("compiler_seen/tests/", "seen_std/tests/")):
        return "unit" if path.endswith(".seen") else ""
    if path.startswith("tests/fixtures/external_package/tests/"):
        return "integration" if path.endswith(".seen") else ""
    if path.startswith("tests/misc_root_tests/"):
        if path.endswith(".sh") or path.endswith("_unit.py") or path.endswith("_test.seen"):
            return "integration"
    return ""


def has_marker(path: str, marker: str) -> bool:
    return f"/{marker}/" in path or f"_{marker}_" in path or f"_{marker}." in path


def expected_platform(path: str) -> str:
    checks = (
        ("linux_arm64", "linux-arm64"),
        ("linux_x86_64", "linux-x86_64"),
        ("windows", "windows"),
        ("macos", "macos"),
        ("linux", "linux"),
    )
    for marker, platform in checks:
        if has_marker(path, marker):
            return platform
    return "all"


def descriptor(path: str) -> dict[str, object]:
    category = expected_category(path)
    if not category:
        fail("invalid", f"unsupported test path: {path}")
    return {
        "category": category,
        "ignored": has_marker(path, "ignored"),
        "path": path,
        "platform": expected_platform(path),
        "privileged": has_marker(path, "privileged"),
        "slow": has_marker(path, "slow"),
    }


def validate(raw: bytes, *, max_bytes: int = MAX_BYTES, max_tests: int = MAX_TESTS,
             cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "test discovery byte limit exceeded")
    if cancelled:
        fail("cancelled", "test discovery cancelled")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {error}")
    if not isinstance(value, dict) or set(value) != FIELDS or value["schema"] != SCHEMA:
        fail("invalid", "invalid test discovery envelope")
    tests = value["tests"]
    if not isinstance(tests, list) or not 0 <= len(tests) <= max_tests <= MAX_TESTS:
        fail("limit", "test discovery item limit exceeded")
    previous = ""
    for entry in tests:
        if not isinstance(entry, dict) or set(entry) != TEST_FIELDS:
            fail("invalid", "invalid test descriptor fields")
        path = entry["path"]
        if not canonical_path(path) or entry != descriptor(path):
            fail("invalid", "test descriptor is not canonical")
        if previous and previous >= path:
            fail("invalid", "test descriptors must be strictly path ordered")
        previous = path
    return {"schema": SCHEMA, "tests": tests}


def discover(repo_root: Path, *, max_tests: int = MAX_TESTS,
             max_bytes: int = MAX_BYTES) -> dict[str, object]:
    if not repo_root.is_absolute() or repo_root.is_symlink() or not repo_root.is_dir():
        fail("invalid", "repository root must be an absolute physical directory")
    try:
        physical_root = repo_root.resolve(strict=True)
    except OSError as error:
        fail("invalid", f"repository root cannot be resolved: {error}")
    if physical_root != repo_root:
        fail("invalid", "repository root contains a symlink or non-canonical segment")
    paths: list[str] = []
    for root_name in ROOTS:
        root = repo_root / root_name
        if root.is_symlink():
            fail("invalid", f"test root is a symlink: {root_name}")
        if not root.exists():
            continue
        for candidate in root.rglob("*"):
            if candidate.is_symlink():
                fail("invalid", f"test tree contains a symlink: {candidate.relative_to(repo_root)}")
            if not candidate.is_file():
                continue
            relative = candidate.relative_to(repo_root).as_posix()
            if expected_category(relative):
                paths.append(relative)
                if len(paths) > max_tests:
                    fail("limit", "test discovery item limit exceeded")
    paths.sort(key=lambda value: value.encode())
    manifest = {"schema": SCHEMA, "tests": [descriptor(path) for path in paths]}
    validate(canonical_bytes(manifest), max_tests=max_tests, max_bytes=max_bytes)
    return manifest


def canonical_bytes(manifest: dict[str, object]) -> bytes:
    return (json.dumps(manifest, separators=(",", ":"), sort_keys=True) + "\n").encode()


def fuzz(raw: bytes, seconds: float, seed: int) -> None:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        mutated = bytearray(raw)
        if mutated:
            mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            validate(bytes(mutated))
        except TestDiscoveryError:
            pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--discover", type=Path)
    action.add_argument("--validate", type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_BYTES)
    parser.add_argument("--max-tests", type=int, default=MAX_TESTS)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        if args.discover:
            discovery_root = args.discover
            if not discovery_root.is_absolute():
                discovery_root = Path.cwd() / discovery_root
            result = discover(discovery_root, max_tests=args.max_tests,
                              max_bytes=args.max_bytes)
            raw = canonical_bytes(result)
        else:
            raw = args.validate.read_bytes()
            result = validate(raw, max_bytes=args.max_bytes,
                              max_tests=args.max_tests,
                              cancelled=args.test_cancel_after_read)
        if args.fuzz_seconds:
            fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"test-discovery: fuzz seed={args.seed} status=pass", file=sys.stderr)
        sys.stdout.buffer.write(canonical_bytes(result))
    except TestDiscoveryError as error:
        print(f"test.001a.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"test.001a.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
