#!/usr/bin/env python3
"""Validate canonical CORE-REL-002 build-instrumentation evidence."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

MAX_BYTES = 65_536
FIELDS = {"components", "modes", "schema", "target"}
COMPONENTS = {"abi_shims", "compiler_host", "native_runtime", "seen_modules"}
MODES = {"coverage", "debug", "sanitizer"}
EVIDENCE = {"source-only", "compile-only", "hardware-executed"}
SANITIZERS = {"", "address", "undefined", "thread", "memory"}


class InstrumentationError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise InstrumentationError(code, message)


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail("invalid", f"duplicate field: {key}")
        result[key] = value
    return result


def validate(raw: bytes, *, max_bytes: int = MAX_BYTES,
             cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "instrumentation evidence byte limit exceeded")
    if cancelled:
        fail("cancelled", "instrumentation evidence validation cancelled")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid instrumentation JSON: {error}")
    if not isinstance(value, dict) or set(value) != FIELDS:
        fail("invalid", "evidence has missing or unknown fields")
    if value["schema"] != "seen-build-instrumentation-evidence-v1" or \
            value["target"] != "linux-x86_64":
        fail("platform", "unsupported schema or execution target")
    components = value["components"]
    modes = value["modes"]
    if not isinstance(components, dict) or set(components) != COMPONENTS or \
            any(item not in EVIDENCE for item in components.values()):
        fail("invalid", "invalid component evidence")
    if not isinstance(modes, dict) or set(modes) != MODES or \
            not isinstance(modes["debug"], bool) or \
            not isinstance(modes["coverage"], bool) or \
            modes["sanitizer"] not in SANITIZERS:
        fail("invalid", "invalid instrumentation modes")
    if not modes["debug"] and not modes["coverage"] and not modes["sanitizer"]:
        fail("invalid", "no instrumentation mode is enabled")
    if any(item == "hardware-executed" for item in components.values()):
        fail("invalid", "compiler builds cannot self-certify hardware execution")
    for name in ("abi_shims", "native_runtime", "seen_modules"):
        if components[name] != "compile-only":
            fail("invalid", f"{name} must carry compile-only evidence")
    if components["compiler_host"] not in {"source-only", "compile-only"}:
        fail("invalid", "compiler host evidence is invalid")
    return {
        "components": {key: components[key] for key in sorted(COMPONENTS)},
        "modes": {key: modes[key] for key in sorted(MODES)},
        "schema": value["schema"], "target": value["target"],
    }


def fuzz(raw: bytes, seconds: float, seed: int) -> None:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        mutated = bytearray(raw)
        if mutated:
            mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            validate(bytes(mutated))
        except InstrumentationError:
            pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "fuzz duration is outside the bounded range")
        raw = args.evidence.read_bytes()
        rendered = validate(raw, max_bytes=args.max_bytes,
                            cancelled=args.test_cancel_after_read)
        if args.fuzz_seconds:
            fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"build-instrumentation: fuzz seed={args.seed} status=pass",
                  file=sys.stderr)
        json.dump(rendered, sys.stdout, separators=(",", ":"), sort_keys=True)
        sys.stdout.write("\n")
    except InstrumentationError as error:
        print(f"core.rel.002.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.rel.002.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
