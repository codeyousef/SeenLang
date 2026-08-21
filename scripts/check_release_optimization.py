#!/usr/bin/env python3
"""Validate canonical CORE-REL-003 release optimization plans."""

from __future__ import annotations
import argparse, json, random, sys, time
from pathlib import Path

MAX_BYTES = 65_536
FIELDS = {"lto_mode", "pgo_mode", "pgo_path", "release", "schema", "target"}

class ReleaseOptimizationError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message); self.code = code

def fail(code: str, message: str) -> None:
    raise ReleaseOptimizationError(code, message)

def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result: fail("invalid", f"duplicate field: {key}")
        result[key] = value
    return result

def canonical_path(value: object) -> bool:
    return isinstance(value, str) and bool(value) and len(value.encode()) <= 4096 \
        and not value.startswith("/") and not value.endswith("/") \
        and ".." not in value and "//" not in value \
        and all(c.isascii() and (c.isalnum() or c in "-._/") for c in value)

def validate(raw: bytes, *, max_bytes: int = MAX_BYTES,
             cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_BYTES or len(raw) > max_bytes:
        fail("limit", "release optimization byte limit exceeded")
    if cancelled: fail("cancelled", "release optimization validation cancelled")
    try: value = json.loads(raw.decode(), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {error}")
    if not isinstance(value, dict) or set(value) != FIELDS:
        fail("invalid", "plan has missing or unknown fields")
    if value["schema"] != "seen-release-optimization-plan-v1" or \
            value["target"] != "linux-x86_64":
        fail("platform", "unsupported schema or target")
    if value["release"] is not True or value["lto_mode"] not in {"full", "thin"}:
        fail("invalid", "invalid release or LTO mode")
    if value["pgo_mode"] not in {"", "generate", "use"}:
        fail("invalid", "invalid PGO mode")
    path = value["pgo_path"]
    if value["pgo_mode"] == "use":
        if not canonical_path(path) or not path.endswith(".profdata"):
            fail("invalid", "PGO use requires a canonical .profdata path")
    elif path != "": fail("invalid", "unexpected PGO path")
    return {key: value[key] for key in sorted(FIELDS)}

def fuzz(raw: bytes, seconds: float, seed: int) -> None:
    rng = random.Random(seed); deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        mutated = bytearray(raw)
        if mutated: mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try: validate(bytes(mutated))
        except ReleaseOptimizationError: pass

def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plan", type=Path); parser.add_argument("--max-bytes", type=int, default=MAX_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0); parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true"); args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300: fail("limit", "invalid fuzz duration")
        raw = args.plan.read_bytes(); result = validate(raw, max_bytes=args.max_bytes, cancelled=args.test_cancel_after_read)
        if args.fuzz_seconds:
            fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"release-optimization: fuzz seed={args.seed} status=pass", file=sys.stderr)
        json.dump(result, sys.stdout, separators=(",", ":"), sort_keys=True); sys.stdout.write("\n")
    except ReleaseOptimizationError as error:
        print(f"core.rel.003.{error.code}: {error}", file=sys.stderr); return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.rel.003.io: {error}", file=sys.stderr); return 1
    return 0

if __name__ == "__main__": sys.exit(main())
