#!/usr/bin/env python3
"""Validate and canonically render the test oracle for package-layout v1."""

from __future__ import annotations

import argparse
import json
import os
import random
import sys
import time
from pathlib import Path

MAX_LAYOUT_BYTES = 65_536
TOP_FIELDS = {
    "examples_root",
    "library_entry",
    "license",
    "manifest",
    "platforms",
    "readme",
    "schema",
    "source_root",
    "tests_root",
}
EXPECTED_PATHS = {
    "examples_root": "examples",
    "library_entry": "src/mod.seen",
    "license": "LICENSE",
    "manifest": "Seen.toml",
    "readme": "README.md",
    "schema": "seen-package-layout-v1",
    "source_root": "src",
    "tests_root": "tests",
}
EXPECTED_PLATFORMS = {
    "linux-arm64": "declared-toolchain-dependent",
    "linux-x86_64": "required",
    "macos": "declared-toolchain-dependent",
    "windows": "declared-toolchain-dependent",
}


class ContractError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise ContractError(code, message)


def object_without_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail("invalid", f"duplicate field {key!r}")
        result[key] = value
    return result


def exact_object(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        fail("invalid", f"{label} has missing or unknown fields")
    return value


def validate(document: object) -> dict[str, object]:
    layout = exact_object(document, TOP_FIELDS, "layout")
    for field, expected in EXPECTED_PATHS.items():
        value = layout[field]
        if not isinstance(value, str) or value != expected:
            fail("invalid", f"{field} is not canonical")
    platforms = exact_object(
        layout["platforms"], set(EXPECTED_PLATFORMS), "platforms"
    )
    for platform, expected in EXPECTED_PLATFORMS.items():
        if platforms[platform] != expected:
            fail("platform", f"{platform} applicability is invalid")
    return layout


def parse_and_validate(raw: bytes, maximum: int) -> dict[str, object]:
    if maximum < 1 or maximum > MAX_LAYOUT_BYTES or len(raw) > maximum:
        fail("limit", "layout byte limit exceeded")
    if raw.startswith(b"\xef\xbb\xbf"):
        fail("invalid", "layout must not contain a UTF-8 BOM")
    try:
        text = raw.decode("utf-8")
        document = json.loads(text, object_pairs_hook=object_without_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"layout JSON is invalid: {error}")
    return validate(document)


def fuzz(corpus: bytes, seconds: float, seed: int, maximum: int) -> None:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        mutated = bytearray(corpus)
        mutation_count = 1 + rng.randrange(4)
        for _ in range(mutation_count):
            action = rng.randrange(3)
            if action == 0 and mutated:
                del mutated[rng.randrange(len(mutated))]
            elif action == 1 and len(mutated) < maximum:
                mutated.insert(rng.randrange(len(mutated) + 1), rng.randrange(256))
            elif mutated:
                mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            parsed = parse_and_validate(bytes(mutated), maximum)
            canonical = (json.dumps(parsed, indent=2, sort_keys=True) + "\n").encode()
            parse_and_validate(canonical, maximum)
        except ContractError:
            pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("layout", type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_LAYOUT_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "fuzz-seconds must be between 0 and 300")
        raw = args.layout.read_bytes()
        if args.test_cancel_after_read:
            if os.environ.get("SEEN_PKG_LAYOUT_TEST_HOOKS") != "1":
                fail("invalid", "cancellation hook is test-only")
            raise ContractError("cancelled", "layout validation cancelled")
        layout = parse_and_validate(raw, args.max_bytes)
        if args.fuzz_seconds:
            fuzz(raw, args.fuzz_seconds, args.seed, args.max_bytes)
            print(
                f"package-layout: fuzz seed={args.seed} "
                f"seconds={args.fuzz_seconds:g} status=pass",
                file=sys.stderr,
            )
        json.dump(layout, sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
    except ContractError as error:
        print(f"pkg.layout.001.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"pkg.layout.001.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
