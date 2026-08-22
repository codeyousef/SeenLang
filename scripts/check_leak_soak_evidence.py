#!/usr/bin/env python3
"""Validate canonical TEST-002D resource-provider leak and soak evidence."""

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-test-leak-soak-v1"
TARGET = "linux-x86_64"
MIN_ITERATIONS = 10_000
MAX_ITERATIONS = 1_000_000
MAX_VALUE = 1_000_000_000_000_000
MAX_BYTES = 1_048_576
PROVIDERS = (
    "allocations", "async-io-completions", "async-io-requests",
    "child-processes", "committed-vram-bytes", "file-descriptors",
    "gpu-objects", "mapped-windows", "persistent-tasks",
    "persistent-workers", "resident-vram-bytes", "staging-bytes", "threads",
)
FIELDS = {"completed_iterations", "planned_iterations", "providers", "schema", "target"}
READING_FIELDS = {"acquired", "available", "baseline", "final", "peak", "provider", "released"}


class ContractError(ValueError):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def fail(code, message):
    raise ContractError(code, message)


def pairs(items):
    output = {}
    for key, value in items:
        if key in output:
            fail("invalid", f"duplicate field: {key}")
        output[key] = value
    return output


def integer(value):
    return isinstance(value, int) and not isinstance(value, bool)


def validate_reading(reading, expected):
    if not isinstance(reading, dict) or set(reading) != READING_FIELDS:
        fail("invalid", "resource provider fields are invalid")
    if reading["provider"] != expected:
        fail("invalid", "resource providers are not canonical")
    if not isinstance(reading["available"], bool):
        fail("invalid", "resource provider availability is not Boolean")
    metrics = ("baseline", "peak", "final", "acquired", "released")
    if any(not integer(reading[key]) for key in metrics):
        fail("invalid", "resource provider metrics must be integers")
    if any(not 0 <= reading[key] <= MAX_VALUE for key in metrics):
        fail("limit", "resource provider metric exceeded bounds")
    if not reading["available"]:
        if any(reading[key] != 0 for key in metrics):
            fail("invalid", "unsupported resource provider reported measurements")
        return reading
    if reading["peak"] < reading["baseline"] or reading["peak"] < reading["final"]:
        fail("invalid", "resource provider peak is inconsistent")
    if reading["acquired"] > 0 and reading["peak"] <= reading["baseline"]:
        fail("invalid", "resource provider churn has no observed peak")
    if reading["baseline"] + reading["acquired"] != reading["final"] + reading["released"]:
        fail("invalid", "resource provider lifecycle is inconsistent")
    if reading["final"] != reading["baseline"]:
        fail("leak", f"resource provider retained resources: {expected}")
    return reading


def validate(raw, max_bytes=MAX_BYTES, cancelled=False):
    if not integer(max_bytes) or not 1 <= max_bytes <= MAX_BYTES:
        fail("limit", "invalid leak and soak evidence byte limit")
    if len(raw) > max_bytes:
        fail("limit", "leak and soak evidence byte limit exceeded")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {type(error).__name__}")
    if cancelled:
        fail("cancelled", "leak and soak validation was cancelled")
    if not isinstance(value, dict) or set(value) != FIELDS:
        fail("invalid", "leak and soak evidence fields are invalid")
    if value["schema"] != SCHEMA:
        fail("invalid", "leak and soak schema is invalid")
    if value["target"] != TARGET:
        fail("platform", "leak and soak target is unsupported")
    planned = value["planned_iterations"]
    completed = value["completed_iterations"]
    if not integer(planned) or not integer(completed):
        fail("invalid", "leak and soak iteration counts must be integers")
    if not MIN_ITERATIONS <= planned <= MAX_ITERATIONS or completed != planned:
        fail("limit", "leak and soak iteration bounds are invalid")
    providers = value["providers"]
    if not isinstance(providers, list) or len(providers) != len(PROVIDERS):
        fail("limit", "resource provider count is invalid")
    available = 0
    for reading, expected in zip(providers, PROVIDERS):
        validate_reading(reading, expected)
        if reading["available"]:
            available += 1
    if available < 1:
        fail("invalid", "no resource provider was available")
    committed = providers[4]
    resident = providers[10]
    if committed["available"] != resident["available"]:
        fail("invalid", "VRAM provider availability is inconsistent")
    if committed["available"]:
        for key in ("baseline", "peak", "final"):
            if resident[key] > committed[key]:
                fail("invalid", "resident VRAM exceeded committed VRAM")
    return value


def canonical(value):
    checked = validate(json.dumps(value).encode())
    return (json.dumps(checked, separators=(",", ":"), sort_keys=True) + "\n").encode()


def fuzz(raw, seconds, seed):
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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--max-bytes", type=int, default=MAX_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        metadata = args.evidence.lstat()
        if args.evidence.is_symlink() or not args.evidence.is_file():
            fail("limit", "leak and soak evidence file is unsafe")
        if metadata.st_size > args.max_bytes:
            fail("limit", "leak and soak evidence file is oversized")
        raw = args.evidence.read_bytes()
        value = validate(raw, args.max_bytes, args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical(value))
    except ContractError as error:
        print(f"test.002d.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"test.002d.io: {type(error).__name__}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
