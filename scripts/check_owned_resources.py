#!/usr/bin/env python3
"""Validate deterministic single-owner resource-transfer traces."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-owned-resource-v1"
MAX_BYTES = 1_048_576
MAX_RESOURCES = 4096
MAX_OPERATIONS = 16_384
FIELDS = {"max_resources", "operations", "schema"}
OP_FIELDS = {"kind", "owner", "resource", "target"}


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


def identity(value: object, *, empty: bool = False) -> bool:
    if empty and value == "":
        return True
    return isinstance(value, str) and 0 < len(value) <= 128 and value.isascii() and all(
        character.islower() or character.isdigit() or character in "-._"
        for character in value
    )


def validate(raw: bytes, max_bytes: int = MAX_BYTES, cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "ownership trace byte limit exceeded")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {error}")
    if not isinstance(value, dict) or set(value) != FIELDS or value.get("schema") != SCHEMA:
        fail("invalid", "invalid ownership trace descriptor")
    if cancelled:
        fail("cancelled", "ownership trace operation is cancelled")
    maximum = value["max_resources"]
    operations = value["operations"]
    if isinstance(maximum, bool) or not isinstance(maximum, int) or not 1 <= maximum <= MAX_RESOURCES:
        fail("limit", "resource bound is invalid")
    if not isinstance(operations, list) or not 1 <= len(operations) <= MAX_OPERATIONS:
        fail("limit", "operation count is invalid")
    for operation in operations:
        if not isinstance(operation, dict) or set(operation) != OP_FIELDS:
            fail("invalid", "ownership operation has missing or unknown fields")
        if operation["kind"] not in ("acquire", "move", "use", "release"):
            fail("invalid", "ownership operation kind is invalid")
        if not identity(operation["owner"]) or not identity(operation["resource"]) or not identity(operation["target"], empty=True):
            fail("invalid", "ownership identity is invalid")
        if operation["kind"] == "move":
            if operation["target"] == "" or operation["target"] == operation["owner"]:
                fail("invalid", "move requires a distinct target owner")
        elif operation["target"] != "":
            fail("invalid", "only move may specify a target owner")
    return value


def evaluate(value: dict[str, object]) -> dict[str, object]:
    owners: dict[str, str | None] = {}
    active = peak = moves = releases = 0
    for index, operation in enumerate(value["operations"]):
        kind = operation["kind"]
        resource = operation["resource"]
        owner = operation["owner"]
        target = operation["target"]
        current = owners.get(resource)
        if kind == "acquire":
            if resource in owners:
                fail("invalid", f"operation {index}: resource was already acquired")
            if active >= value["max_resources"]:
                fail("limit", f"operation {index}: active resource bound exceeded")
            owners[resource] = owner
            active += 1
            peak = max(peak, active)
        elif current is None:
            fail("invalid", f"operation {index}: resource is not active")
        elif current != owner:
            fail("use-after-move", f"operation {index}: owner no longer owns resource")
        elif kind == "move":
            owners[resource] = target
            moves += 1
        elif kind == "release":
            owners[resource] = None
            active -= 1
            releases += 1
    leaked = sorted(resource for resource, owner in owners.items() if owner is not None)
    if leaked:
        fail("leak", "active resources remain at end of trace: " + ",".join(leaked))
    return {
        "active": active,
        "moves": moves,
        "operations": len(value["operations"]),
        "peak": peak,
        "released": releases,
        "schema": SCHEMA,
    }


def canonical_bytes(value: dict[str, object]) -> bytes:
    return (json.dumps(evaluate(value), separators=(",", ":"), sort_keys=True) + "\n").encode()


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
            evaluate(validate(bytes(changed)))
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
        print(f"p0.own.001.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"p0.own.001.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
