#!/usr/bin/env python3
"""Validate the deterministic ERR-001D error-policy classifier."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-error-policy-v1"
MAX_BYTES = 1_048_576
MAX_MESSAGE = 4096
MAX_ATTEMPTS = 1024
FIELDS = {"attempt", "error", "max_attempts", "schema"}
ERROR_FIELDS = {"code", "message", "operation", "redaction", "retry", "subsystem"}


class ContractError(ValueError):
    def __init__(self, code: str, message: str):
        super().__init__(message); self.code = code


def fail(code: str, message: str) -> None:
    raise ContractError(code, message)


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value: fail("invalid", f"duplicate field: {key}")
        value[key] = item
    return value


def identity(value: object) -> bool:
    return isinstance(value, str) and 0 < len(value) <= 128 and value.isascii() and all(
        character.islower() or character.isdigit() or character in "-._" for character in value
    )


def validate(raw: bytes, max_bytes: int = MAX_BYTES, cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "error-policy byte limit exceeded")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {error}")
    if not isinstance(value, dict) or set(value) != FIELDS or value.get("schema") != SCHEMA:
        fail("invalid", "invalid error-policy descriptor")
    if cancelled: fail("cancelled", "error-policy operation is cancelled")
    attempt = value["attempt"]
    maximum = value["max_attempts"]
    if any(isinstance(item, bool) or not isinstance(item, int) for item in (attempt, maximum)) or not 0 <= attempt <= maximum <= MAX_ATTEMPTS or maximum < 1:
        fail("limit", "retry attempt bound is invalid")
    error = value["error"]
    if not isinstance(error, dict) or set(error) != ERROR_FIELDS:
        fail("invalid", "invalid structured error")
    if not all(identity(error[field]) for field in ("code", "operation", "subsystem")):
        fail("invalid", "error identity is invalid")
    message = error["message"]
    if not isinstance(message, str): fail("invalid", "error message must be text")
    if len(message.encode()) > MAX_MESSAGE: fail("limit", "error message exceeds its bound")
    if error["retry"] not in ("never", "transient") or error["redaction"] not in ("public", "sensitive"):
        fail("invalid", "error retry or redaction policy is invalid")
    is_cancelled = error["subsystem"] == "cancelled" or error["code"].endswith(".cancelled")
    if is_cancelled and error["retry"] != "never":
        fail("invalid", "cancelled errors cannot be retryable")
    return value


def classify(value: dict[str, object]) -> dict[str, object]:
    error = value["error"]
    cancelled = error["subsystem"] == "cancelled" or error["code"].endswith(".cancelled")
    if cancelled:
        disposition = "cancelled"
    elif error["retry"] == "transient":
        disposition = "exhausted" if value["attempt"] >= value["max_attempts"] else "retry"
    else:
        disposition = "permanent"
    redacted = error["redaction"] == "sensitive"
    return {
        "attempt": value["attempt"], "disposition": disposition,
        "max_attempts": value["max_attempts"],
        "message": "[redacted]" if redacted else error["message"],
        "redacted": redacted, "schema": SCHEMA,
    }


def canonical_bytes(value: dict[str, object]) -> bytes:
    return (json.dumps(classify(value), separators=(",", ":"), sort_keys=True) + "\n").encode()


def fuzz(raw: bytes, seconds: float, seed: int) -> tuple[int, int]:
    rng = random.Random(seed); deadline = time.monotonic() + seconds
    cases = rejected = 0
    while time.monotonic() < deadline:
        changed = bytearray(raw)
        if changed: changed[rng.randrange(len(changed))] = rng.randrange(256)
        cases += 1
        try: validate(bytes(changed))
        except ContractError: rejected += 1
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
        if not 0 <= args.fuzz_seconds <= 300: fail("limit", "invalid fuzz duration")
        raw = args.validate.read_bytes()
        value = validate(raw, args.max_bytes, args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical_bytes(value))
    except ContractError as error:
        print(f"err.001d.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"err.001d.io: {error}", file=sys.stderr); return 1
    return 0


if __name__ == "__main__": sys.exit(main())
