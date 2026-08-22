#!/usr/bin/env python3
"""Validate and render bounded redaction-safe secret marker descriptors."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-secret-marker-v1"
REDACTED = "[redacted]"
MAX_BYTES = 1_048_576
MAX_FIELDS = 4096
MAX_SECRET_BYTES = 4096
ROOT_FIELDS = {"fields", "max_secret_bytes", "schema"}
FIELD_FIELDS = {"kind", "name", "value"}


class ContractError(ValueError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise ContractError(code, message)


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail("invalid", f"duplicate field: {key}")
        result[key] = value
    return result


def identity(value: object) -> bool:
    return isinstance(value, str) and 0 < len(value) <= 128 and value.isascii() and all(
        character.islower() or character.isdigit() or character in "-._"
        for character in value
    )


def validate(raw: bytes, max_bytes: int = MAX_BYTES, cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "secret marker descriptor byte limit exceeded")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {error.__class__.__name__}")
    if not isinstance(value, dict) or set(value) != ROOT_FIELDS or value.get("schema") != SCHEMA:
        fail("invalid", "invalid secret marker descriptor")
    if cancelled:
        fail("cancelled", "secret marker operation is cancelled")
    maximum = value["max_secret_bytes"]
    fields = value["fields"]
    if isinstance(maximum, bool) or not isinstance(maximum, int) or not 1 <= maximum <= MAX_SECRET_BYTES:
        fail("limit", "secret value bound is invalid")
    if not isinstance(fields, list) or not 1 <= len(fields) <= MAX_FIELDS:
        fail("limit", "secret marker field count is invalid")
    names: set[str] = set()
    for field in fields:
        if not isinstance(field, dict) or set(field) != FIELD_FIELDS:
            fail("invalid", "secret marker has missing or unknown fields")
        kind, name, material = field["kind"], field["name"], field["value"]
        if kind not in ("public", "secret") or not identity(name):
            fail("invalid", "secret marker kind or identity is invalid")
        if name in names:
            fail("invalid", "secret marker identity is duplicated")
        names.add(name)
        if not isinstance(material, str) or not material or len(material.encode()) > maximum:
            fail("limit", "secret marker value exceeds its bound")
    return value


def canonical_bytes(value: dict[str, object]) -> bytes:
    fields = sorted(value["fields"], key=lambda field: field["name"].encode())
    rendered = [
        {
            "kind": field["kind"],
            "name": field["name"],
            "value": REDACTED if field["kind"] == "secret" else field["value"],
        }
        for field in fields
    ]
    output = {
        "fields": rendered,
        "redacted": sum(field["kind"] == "secret" for field in fields),
        "schema": SCHEMA,
    }
    return (json.dumps(output, separators=(",", ":"), sort_keys=True) + "\n").encode()


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
            canonical_bytes(validate(bytes(changed)))
        except ContractError:
            rejected += 1
    return cases, rejected


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--validate", required=True, type=Path)
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
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical_bytes(value))
    except ContractError as error:
        print(f"p0.secret.001.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"p0.secret.001.io: {error.__class__.__name__}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
