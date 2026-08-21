#!/usr/bin/env python3
"""Validate a bounded production-source handoff without rewriting its bytes."""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import sys
import time
from pathlib import Path

MAX_INPUT_BYTES = 1_048_576
MAX_SOURCES = 4096
MAX_PATH_BYTES = 4096
TOP_FIELDS = {"platform", "rewrite_requested", "schema", "sources"}
SOURCE_FIELDS = {"checkout_sha256", "compiler_input_sha256", "path"}
KNOWN_PLATFORMS = {
    "android-arm64", "ios-arm64", "ios-sim-arm64", "linux-arm64",
    "linux-riscv64", "linux-x86_64", "macos", "macos-arm64",
    "macos-x86_64", "windows", "windows-x86_64",
}
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


class PolicyError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise PolicyError(code, message)


def object_without_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            fail("invalid", f"duplicate JSON field: {key}")
        value[key] = item
    return value


def exact_object(value: object, fields: set[str], name: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        fail("invalid", f"{name} has missing or unknown fields")
    return value


def canonical_path(value: object, max_path_bytes: int) -> str:
    if not isinstance(value, str):
        fail("invalid", "production source path is not a string")
    encoded = value.encode("utf-8")
    if not encoded or len(encoded) > max_path_bytes:
        fail("limit", "production source path byte limit exceeded")
    if "\x00" in value or "\\" in value or value.startswith("/"):
        fail("invalid", "production source path is not canonical")
    if any(segment in {"", ".", ".."} for segment in value.split("/")):
        fail("invalid", "production source path is not canonical")
    return value


def parse_and_validate(
    raw: bytes,
    max_bytes: int = MAX_INPUT_BYTES,
    max_sources: int = MAX_SOURCES,
    max_path_bytes: int = MAX_PATH_BYTES,
    cancelled: bool = False,
) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_INPUT_BYTES or len(raw) > max_bytes:
        fail("limit", "production source input byte limit exceeded")
    if not 1 <= max_sources <= MAX_SOURCES or not 1 <= max_path_bytes <= MAX_PATH_BYTES:
        fail("limit", "production source limits are invalid")
    if cancelled:
        fail("cancelled", "production source validation cancelled")
    if raw.startswith(b"\xef\xbb\xbf"):
        fail("invalid", "production source input must not contain a UTF-8 BOM")
    try:
        document = json.loads(raw.decode("utf-8"), object_pairs_hook=object_without_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"production source JSON is invalid: {error}")
    value = exact_object(document, TOP_FIELDS, "production source input")
    if value["schema"] != "seen-production-source-input-v1":
        fail("invalid", "production source schema is unsupported")
    if value["platform"] not in KNOWN_PLATFORMS:
        fail("platform", "production source platform is unsupported")
    if value["rewrite_requested"] is not False:
        fail("invalid", "production source rewriting is forbidden")
    raw_sources = value["sources"]
    if not isinstance(raw_sources, list):
        fail("invalid", "production sources are not an array")
    if not 1 <= len(raw_sources) <= max_sources:
        fail("limit", "production source file limit exceeded")
    paths: list[str] = []
    for raw_source in raw_sources:
        source = exact_object(raw_source, SOURCE_FIELDS, "production source")
        path = canonical_path(source["path"], max_path_bytes)
        if path in paths:
            fail("invalid", "production source path is duplicated")
        checkout = source["checkout_sha256"]
        compiler_input = source["compiler_input_sha256"]
        if not isinstance(checkout, str) or not SHA256_RE.fullmatch(checkout):
            fail("invalid", "checked-out source digest is invalid")
        if not isinstance(compiler_input, str) or not SHA256_RE.fullmatch(compiler_input):
            fail("invalid", "compiler-input source digest is invalid")
        if checkout != compiler_input:
            fail("invalid", "compiler input differs from checked-out source")
        paths.append(path)
    paths.sort(key=lambda path: path.encode("utf-8"))
    return {
        "ordered_paths": paths,
        "rewrite_allowed": False,
        "schema": "seen-production-source-policy-v1",
        "source_count": len(paths),
    }


def fuzz(corpus: bytes, seconds: float, seed: int) -> None:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        mutated = bytearray(corpus)
        for _ in range(1 + rng.randrange(4)):
            action = rng.randrange(3)
            if action == 0 and mutated:
                del mutated[rng.randrange(len(mutated))]
            elif action == 1 and len(mutated) < MAX_INPUT_BYTES:
                mutated.insert(rng.randrange(len(mutated) + 1), rng.randrange(256))
            elif mutated:
                mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            json.dumps(parse_and_validate(bytes(mutated)), sort_keys=True)
        except PolicyError:
            pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_INPUT_BYTES)
    parser.add_argument("--max-sources", type=int, default=MAX_SOURCES)
    parser.add_argument("--max-path-bytes", type=int, default=MAX_PATH_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "fuzz-seconds must be between 0 and 300")
        raw = args.input.read_bytes()
        cancelled = args.test_cancel_after_read
        if cancelled and os.environ.get("SEEN_PRODUCTION_SOURCE_TEST_HOOKS") != "1":
            fail("invalid", "cancellation hook is test-only")
        result = parse_and_validate(
            raw, args.max_bytes, args.max_sources, args.max_path_bytes, cancelled)
        if args.fuzz_seconds:
            fuzz(raw, args.fuzz_seconds, args.seed)
            print(
                f"production-source: fuzz seed={args.seed} "
                f"seconds={args.fuzz_seconds:g} status=pass",
                file=sys.stderr,
            )
        json.dump(result, sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
    except PolicyError as error:
        print(f"core.003d.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.003d.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
