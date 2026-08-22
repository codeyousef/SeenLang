#!/usr/bin/env python3
"""Discover and validate the canonical TEST-001F package migration plan."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-test-migration-v1"
MAX_BYTES = 1024 * 1024
MAX_PATH_BYTES = 4096
MAX_TESTS = 100_000
FIELDS = {"entries", "schema", "total_tests"}
ENTRY_FIELDS = {
    "category", "manifest_path", "package_root", "platform", "source",
    "test_count", "tests_root",
}
SOURCES = (
    ("compiler", "compiler_seen", "unit"),
    ("stdlib", "seen_std", "unit"),
    ("external-package", "tests/fixtures/external_package", "integration"),
)


class MigrationError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise MigrationError(code, message)


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail("invalid", f"duplicate field: {key}")
        result[key] = value
    return result


def canonical_path(value: object) -> bool:
    return (
        isinstance(value, str) and bool(value) and value.isascii()
        and len(value.encode()) <= MAX_PATH_BYTES and not value.startswith("/")
        and not value.endswith("/") and ".." not in value
        and "//" not in value and "\\" not in value
        and all(ch.isalnum() or ch in "-._/" for ch in value)
    )


def expected_entry(source: str, root: str, category: str,
                   count: int) -> dict[str, object]:
    return {
        "category": category,
        "manifest_path": f"{root}/Seen.toml",
        "package_root": root,
        "platform": "all",
        "source": source,
        "test_count": count,
        "tests_root": f"{root}/tests",
    }


def canonical_bytes(value: dict[str, object]) -> bytes:
    return (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode()


def validate(raw: bytes, *, max_bytes: int = MAX_BYTES,
             max_tests: int = MAX_TESTS,
             cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "test migration byte limit exceeded")
    if cancelled:
        fail("cancelled", "test migration validation cancelled")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {error}")
    if not isinstance(value, dict) or set(value) != FIELDS or value["schema"] != SCHEMA:
        fail("invalid", "invalid test migration envelope")
    entries = value["entries"]
    if not isinstance(entries, list) or len(entries) != len(SOURCES):
        fail("limit", "test migration source count is not canonical")
    total = 0
    for entry, (source, root, category) in zip(entries, SOURCES, strict=True):
        if not isinstance(entry, dict) or set(entry) != ENTRY_FIELDS:
            fail("invalid", "invalid test migration entry fields")
        count = entry["test_count"]
        if not isinstance(count, int) or isinstance(count, bool) or not 1 <= count <= MAX_TESTS:
            fail("limit", "invalid migrated test count")
        if entry != expected_entry(source, root, category, count):
            fail("invalid", "test migration entry is not canonical")
        if not all(canonical_path(entry[field]) for field in
                   ("manifest_path", "package_root", "tests_root")):
            fail("invalid", "test migration path is unsafe")
        if total > max_tests - count:
            fail("limit", "test migration total exceeds limit")
        total += count
    if value["total_tests"] != total:
        fail("invalid", "test migration total is inconsistent")
    return value


def discover(repo_root: Path, *, max_tests: int = MAX_TESTS,
             max_bytes: int = MAX_BYTES) -> dict[str, object]:
    if not repo_root.is_absolute() or repo_root.is_symlink() or not repo_root.is_dir():
        fail("invalid", "repository root must be an absolute physical directory")
    if repo_root.resolve(strict=True) != repo_root:
        fail("invalid", "repository root is not canonical")
    entries: list[dict[str, object]] = []
    total = 0
    for source, root_name, category in SOURCES:
        package_root = repo_root / root_name
        manifest = package_root / "Seen.toml"
        tests_root = package_root / "tests"
        for path, label in ((package_root, "package root"),
                            (manifest, "package manifest"),
                            (tests_root, "tests root")):
            if path.is_symlink() or not path.exists():
                fail("invalid", f"missing or unsafe {label}: {path.relative_to(repo_root)}")
        if not package_root.is_dir() or not tests_root.is_dir() or not manifest.is_file():
            fail("invalid", f"invalid package topology: {root_name}")
        tests: list[Path] = []
        for candidate in tests_root.rglob("*"):
            if candidate.is_symlink():
                fail("invalid", f"test tree contains symlink: {candidate.relative_to(repo_root)}")
            if candidate.is_file() and candidate.suffix == ".seen":
                tests.append(candidate)
        count = len(tests)
        if count < 1 or total > max_tests - count:
            fail("limit", f"invalid migrated test count: {root_name}")
        total += count
        entries.append(expected_entry(source, root_name, category, count))
    plan = {"entries": entries, "schema": SCHEMA, "total_tests": total}
    validate(canonical_bytes(plan), max_bytes=max_bytes, max_tests=max_tests)
    return plan


def fuzz(raw: bytes, seconds: float, seed: int) -> tuple[int, int]:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = rejected = 0
    while time.monotonic() < deadline:
        mutated = bytearray(raw)
        if mutated:
            mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        cases += 1
        try:
            validate(bytes(mutated))
        except MigrationError:
            rejected += 1
    return cases, rejected


def benchmark(raw: bytes, limit_ms: float) -> None:
    for _ in range(5):
        validate(raw)
    samples: list[float] = []
    for _ in range(30):
        started = time.perf_counter_ns()
        validate(raw)
        samples.append((time.perf_counter_ns() - started) / 1_000_000)
    samples.sort()
    median = samples[len(samples) // 2]
    if median > limit_ms:
        fail("limit", f"benchmark median {median:.6f}ms exceeds {limit_ms:.6f}ms")
    print(f"warmups=5 samples=30 median_ms={median:.6f} status=pass")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--discover", type=Path)
    action.add_argument("--validate", type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_BYTES)
    parser.add_argument("--max-tests", type=int, default=MAX_TESTS)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--benchmark-limit-ms", type=float, default=0.0)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300 or args.benchmark_limit_ms < 0:
            fail("limit", "invalid verification limit")
        if args.discover:
            root = args.discover if args.discover.is_absolute() else Path.cwd() / args.discover
            value = discover(root, max_tests=args.max_tests, max_bytes=args.max_bytes)
            raw = canonical_bytes(value)
        else:
            raw = args.validate.read_bytes()
            value = validate(raw, max_bytes=args.max_bytes, max_tests=args.max_tests,
                             cancelled=args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        if args.benchmark_limit_ms:
            benchmark(raw, args.benchmark_limit_ms)
        else:
            sys.stdout.buffer.write(canonical_bytes(value))
    except MigrationError as error:
        print(f"test.001f.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"test.001f.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
