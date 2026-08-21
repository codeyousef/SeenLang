#!/usr/bin/env python3
"""Strict host oracle for the native seen-test-run-v1 report contract."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-test-run-v1"
TOP_KEYS = {"schema", "results", "passed", "failed", "skipped", "exit_code"}
RESULT_KEYS = {"path", "status", "exit_code"}
STATUSES = {"passed", "failed", "skipped"}


class ContractError(ValueError):
    pass


def canonical_path(value: object) -> bool:
    if not isinstance(value, str) or not value or len(value.encode()) > 4096:
        return False
    if value.startswith("/") or "\\" in value or ".." in value or "//" in value:
        return False
    return all(ch.isascii() and (ch.isalnum() or ch in "-._/") for ch in value)


def validate(payload: object) -> dict[str, object]:
    if not isinstance(payload, dict) or set(payload) != TOP_KEYS:
        raise ContractError("test.001b.invalid: report fields are not canonical")
    if payload["schema"] != SCHEMA or not isinstance(payload["results"], list):
        raise ContractError("test.001b.invalid: report schema is invalid")
    counts = {"passed": 0, "failed": 0, "skipped": 0}
    previous = ""
    for result in payload["results"]:
        if not isinstance(result, dict) or set(result) != RESULT_KEYS:
            raise ContractError("test.001b.invalid: result fields are not canonical")
        path = result["path"]
        status = result["status"]
        code = result["exit_code"]
        if not canonical_path(path) or path <= previous:
            raise ContractError("test.001b.invalid: result path ordering is invalid")
        if status not in STATUSES or type(code) is not int or not 0 <= code <= 255:
            raise ContractError("test.001b.invalid: result status is invalid")
        if (status in {"passed", "skipped"}) != (code == 0):
            raise ContractError("test.001b.invalid: result exit code is inconsistent")
        counts[status] += 1
        previous = path
    for name, expected in counts.items():
        if type(payload[name]) is not int or payload[name] != expected:
            raise ContractError("test.001b.invalid: report counts are inconsistent")
    expected_exit = 1 if counts["failed"] else 0
    if payload["exit_code"] != expected_exit:
        raise ContractError("test.001b.invalid: aggregate exit code is inconsistent")
    return payload


def load(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ContractError("test.001b.io: report input is unavailable") from exc


def fuzz(seconds: float, seed: int) -> int:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = 0
    base = {"schema": SCHEMA, "results": [], "passed": 0, "failed": 0,
            "skipped": 0, "exit_code": 0}
    while time.monotonic() < deadline or cases == 0:
        payload = dict(base)
        del payload[rng.choice(tuple(TOP_KEYS))]
        try:
            validate(payload)
        except ContractError:
            pass
        else:
            raise ContractError("test.001b.invalid: fuzz mutation accepted")
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
        raise ContractError("test.001b.limit: benchmark exceeded hard 5% gate")
    print(f"warmups=5 samples=30 median_ms={measured:.6f} status=pass")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate", type=Path)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--benchmark-limit-ms", type=float)
    args = parser.parse_args()
    if args.fuzz_seconds < 0:
        raise ContractError("test.001b.invalid: negative fuzz duration")
    payload = validate(load(args.validate)) if args.validate else None
    if args.fuzz_seconds:
        fuzz(args.fuzz_seconds, args.seed)
    if args.benchmark_limit_ms is not None:
        if payload is None or args.benchmark_limit_ms <= 0:
            raise ContractError("test.001b.invalid: benchmark input is invalid")
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
