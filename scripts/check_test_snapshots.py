#!/usr/bin/env python3
"""Strict host oracle for canonical Seen snapshot files."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-test-snapshot-v1"
KEYS = {"schema", "name", "content"}
MAX_NAME_BYTES = 128
MAX_CONTENT_BYTES = 1_048_576


class ContractError(ValueError):
    pass


def safe_name(value: object) -> bool:
    return (isinstance(value, str) and value != "" and
            not value.startswith(".") and
            len(value.encode("utf-8")) <= MAX_NAME_BYTES and
            all(ch.isascii() and (ch.isalnum() or ch in "-._")
                for ch in value))


def validate(payload: object) -> dict[str, object]:
    if not isinstance(payload, dict) or set(payload) != KEYS:
        raise ContractError("test.001c.invalid: snapshot fields are not canonical")
    if payload["schema"] != SCHEMA or not safe_name(payload["name"]):
        raise ContractError("test.001c.invalid: snapshot identity is invalid")
    content = payload["content"]
    if not isinstance(content, str):
        raise ContractError("test.001c.invalid: snapshot content is not text")
    if len(content.encode("utf-8")) > MAX_CONTENT_BYTES:
        raise ContractError("test.001c.limit: snapshot content limit exceeded")
    return payload


def load(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ContractError("test.001c.io: snapshot input is unavailable") from exc


def fuzz(seconds: float, seed: int) -> int:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = 0
    base = {"schema": SCHEMA, "name": "case", "content": "value\n"}
    while time.monotonic() < deadline or cases == 0:
        payload = dict(base)
        mutation = rng.randrange(5)
        if mutation == 0:
            del payload[rng.choice(tuple(KEYS))]
        elif mutation == 1:
            payload["schema"] = "unknown"
        elif mutation == 2:
            payload["name"] = "../escape"
        elif mutation == 3:
            payload["content"] = None
        else:
            payload["extra"] = True
        try:
            validate(payload)
        except ContractError:
            pass
        else:
            raise ContractError("test.001c.invalid: fuzz mutation accepted")
        cases += 1
    print(f"seed={seed} cases={cases}", file=sys.stderr)
    return cases


def benchmark(payload: object, limit_ms: float) -> None:
    for _ in range(5):
        validate(payload)
    samples = []
    for _ in range(30):
        start = time.perf_counter_ns()
        validate(payload)
        samples.append((time.perf_counter_ns() - start) / 1_000_000)
    measured = sorted(samples)[len(samples) // 2]
    if measured > limit_ms * 1.05:
        raise ContractError("test.001c.limit: benchmark exceeded hard 5% gate")
    print(f"warmups=5 samples=30 median_ms={measured:.6f} status=pass")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate", type=Path)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--benchmark-limit-ms", type=float)
    args = parser.parse_args()
    if args.fuzz_seconds < 0:
        raise ContractError("test.001c.invalid: negative fuzz duration")
    payload = validate(load(args.validate)) if args.validate else None
    if args.fuzz_seconds:
        fuzz(args.fuzz_seconds, args.seed)
    if args.benchmark_limit_ms is not None:
        if payload is None or args.benchmark_limit_ms <= 0:
            raise ContractError("test.001c.invalid: benchmark input is invalid")
        benchmark(payload, args.benchmark_limit_ms)
    if payload is not None and args.benchmark_limit_ms is None:
        print(json.dumps(payload, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ContractError as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(2)
