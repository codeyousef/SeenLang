#!/usr/bin/env python3
"""Validate the canonical ERR-001B typed-error descriptor."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-typed-error-v1"
MAX_BYTES = 1_048_576
MAX_MESSAGE = 4096
MAX_IDENTITY = 128
FIELDS = {
    "kind",
    "message",
    "native_code",
    "operation",
    "redaction",
    "retry",
    "schema",
}
KINDS = {"os", "io", "process", "network", "timeout", "cancelled", "parse", "resource"}


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
    return (
        isinstance(value, str)
        and 0 < len(value.encode()) <= MAX_IDENTITY
        and value.isascii()
        and all(character.islower() or character.isdigit() or character in "-._" for character in value)
    )


def validate(raw: bytes, max_bytes: int = MAX_BYTES, cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "typed-error byte limit exceeded")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {error}")
    if not isinstance(value, dict) or set(value) != FIELDS or value["schema"] != SCHEMA:
        fail("invalid", "invalid typed-error descriptor")
    if cancelled:
        fail("cancelled", "typed-error operation is cancelled")
    kind = value["kind"]
    if kind not in KINDS or not identity(value["operation"]):
        fail("invalid", "invalid typed-error kind or operation")
    message = value["message"]
    if not isinstance(message, str):
        fail("invalid", "typed-error message must be text")
    if len(message.encode()) > MAX_MESSAGE:
        fail("limit", "typed-error message exceeds limit")
    if value["retry"] not in ("never", "transient") or value["redaction"] not in ("public", "sensitive"):
        fail("invalid", "invalid typed-error policy")
    if kind in ("cancelled", "parse") and value["retry"] != "never":
        fail("invalid", "policy errors cannot be retryable")
    native_code = value["native_code"]
    if native_code is not None and (
        not isinstance(native_code, int)
        or isinstance(native_code, bool)
        or native_code < -(1 << 63)
        or native_code > (1 << 63) - 1
    ):
        fail("invalid", "invalid native error code")
    return value


def canonical_bytes(value: dict[str, object]) -> bytes:
    message = value["message"]
    if value["redaction"] == "sensitive":
        message = "[redacted]"
    rendered = {
        "causes": [],
        "code": f"err.001b.{value['kind']}",
        "message": message,
        "native_code": value["native_code"],
        "operation": value["operation"],
        "redaction": value["redaction"],
        "retry": value["retry"],
        "subsystem": value["kind"],
    }
    return (json.dumps(rendered, separators=(",", ":"), sort_keys=True) + "\n").encode()


def fuzz(raw: bytes, seconds: float, seed: int) -> tuple[int, int]:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = 0
    rejected = 0
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
    parser.add_argument("--max-bytes", default=MAX_BYTES, type=int)
    parser.add_argument("--fuzz-seconds", default=0.0, type=float)
    parser.add_argument("--seed", default=1101, type=int)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        raw = args.validate.read_bytes()
        value = validate(raw, max_bytes=args.max_bytes, cancelled=args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical_bytes(value))
    except ContractError as error:
        print(f"err.001b.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"err.001b.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
