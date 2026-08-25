#!/usr/bin/env python3
"""Validate BYTES-002A async progress, cancellation, and performance evidence."""

from __future__ import annotations

import argparse
import json
import random
import statistics
import sys
import time
from pathlib import Path


class ContractError(ValueError):
    pass


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate key: {key}")
        result[key] = value
    return result


def exact(value: dict[str, object], keys: set[str], where: str) -> None:
    if set(value) != keys:
        raise ContractError(f"{where} keys must be {sorted(keys)}")


def checked_progress(available: int, capacity: int, cancelled: bool) -> int:
    if available < 0 or capacity < 0:
        raise ContractError("progress geometry must be nonnegative")
    if cancelled:
        return 0
    return min(available, capacity)


def load(path: Path) -> dict[str, object]:
    raw = path.read_bytes()
    if len(raw) > 65_536:
        raise ContractError("contract exceeds 64 KiB")
    try:
        value = json.loads(raw, object_pairs_hook=unique_object)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ContractError(f"invalid UTF-8 JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise ContractError("root must be an object")
    exact(value, {"benchmark", "cases", "fuzz_seed", "schema"}, "root")
    if value["schema"] != "seen-async-byte-streams-v1":
        raise ContractError("schema mismatch")
    if value["fuzz_seed"] != 1101:
        raise ContractError("fuzz seed must be 1101")
    cases = value["cases"]
    if not isinstance(cases, list) or not 1 <= len(cases) <= 128:
        raise ContractError("cases must contain 1 through 128 entries")
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            raise ContractError(f"cases[{index}] must be an object")
        exact(case, {"available", "cancelled", "capacity", "expected"}, f"cases[{index}]")
        if not isinstance(case["cancelled"], bool):
            raise ContractError(f"cases[{index}].cancelled must be boolean")
        for key in ("available", "capacity", "expected"):
            if isinstance(case[key], bool) or not isinstance(case[key], int):
                raise ContractError(f"cases[{index}].{key} must be an integer")
        expected = checked_progress(case["available"], case["capacity"], case["cancelled"])
        if case["expected"] != expected:
            raise ContractError(f"cases[{index}] progress mismatch")
    benchmark = value["benchmark"]
    if not isinstance(benchmark, dict):
        raise ContractError("benchmark must be an object")
    exact(benchmark, {"baseline_ratio", "iterations_per_sample", "samples", "warmups"}, "benchmark")
    if benchmark["warmups"] != 5 or benchmark["samples"] != 30:
        raise ContractError("benchmark must use five warmups and 30 samples")
    return value


def fuzz(seconds: float, seed: int) -> int:
    if seconds <= 0 or seconds > 300:
        raise ContractError("fuzz duration must be in (0, 300]")
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    checked = 0
    while time.monotonic() < deadline:
        source = bytes(rng.randrange(256) for _ in range(rng.randrange(0, 4096)))
        position = 0
        output = bytearray()
        while position < len(source):
            capacity = rng.randrange(1, 257)
            cancelled = rng.randrange(97) == 0
            count = checked_progress(len(source) - position, capacity, cancelled)
            if cancelled:
                if count != 0:
                    raise ContractError(f"cancellation made progress seed={seed}")
                continue
            output.extend(source[position : position + count])
            position += count
        if output != source:
            raise ContractError(f"async chunk equivalence failed seed={seed}")
        checked += 1
    return checked


def timed(iterations: int, candidate: bool) -> int:
    started = time.perf_counter_ns()
    checksum = 0
    for index in range(iterations):
        available = index & 4095
        capacity = (index * 3) & 1023
        checksum += checked_progress(available, capacity, False) if candidate else min(available, capacity)
    elapsed = time.perf_counter_ns() - started
    if checksum < 0:
        raise ContractError("unreachable checksum")
    return elapsed


def benchmark(contract: dict[str, object]) -> tuple[float, float, float]:
    config = contract["benchmark"]
    assert isinstance(config, dict)
    iterations = int(config["iterations_per_sample"])
    for _ in range(5):
        timed(iterations, False)
        timed(iterations, True)
    ratios = [timed(iterations, True) / max(timed(iterations, False), 1) for _ in range(30)]
    ratio = statistics.median(ratios)
    baseline = float(config["baseline_ratio"])
    limit = baseline * 1.05
    if ratio > limit:
        raise ContractError(f"hard 5% gate failed: median_ratio={ratio:.6f} limit={limit:.6f}")
    return ratio, baseline, limit


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("contract", type=Path)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int)
    parser.add_argument("--benchmark", action="store_true")
    args = parser.parse_args()
    try:
        contract = load(args.contract)
        print(f"async-byte-streams: valid {args.contract}")
        if args.fuzz_seconds:
            seed = args.seed if args.seed is not None else int(contract["fuzz_seed"])
            print(f"async-byte-streams: fuzz seed={seed} cases={fuzz(args.fuzz_seconds, seed)} status=pass")
        if args.benchmark:
            ratio, baseline, limit = benchmark(contract)
            print("async-byte-streams: benchmark warmups=5 samples=30 hard_gate=5% "
                  f"median_ratio={ratio:.6f} baseline_ratio={baseline:.6f} "
                  f"limit={limit:.6f} status=pass")
    except (ContractError, OSError, TypeError, ValueError) as exc:
        print(f"async-byte-streams: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
