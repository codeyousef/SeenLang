#!/usr/bin/env python3
"""Validate and stress the bounded BYTES-001A geometry contract."""

from __future__ import annotations

import argparse
import json
import random
import statistics
import sys
import time
from pathlib import Path

MAX_BYTES = 65_536
MAX_CASES = 128
MAX_COUNT = (1 << 63) - 1


class ContractError(ValueError):
    pass


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ContractError(f"duplicate key: {key}")
        value[key] = item
    return value


def exact_keys(value: dict[str, object], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise ContractError(f"{where} keys must be {sorted(expected)}")


def bounded_int(value: object, where: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ContractError(f"{where} must be an integer")
    if value < minimum or value > maximum:
        raise ContractError(f"{where} is outside [{minimum}, {maximum}]")
    return value


def validate_range(total: int, offset: int, count: int) -> str:
    if total < 0 or offset < 0 or count < 0:
        return "byte.invalid"
    if total > MAX_COUNT or offset > total or count > total - offset:
        return "byte.limit"
    return "ok"


def reference_valid(total: int, offset: int, count: int) -> bool:
    return (
        0 <= total <= MAX_COUNT
        and 0 <= offset <= total
        and 0 <= count <= total - offset
    )


def load_contract(path: Path) -> dict[str, object]:
    raw = path.read_bytes()
    if len(raw) > MAX_BYTES:
        raise ContractError("contract exceeds 64 KiB")
    try:
        value = json.loads(raw, object_pairs_hook=unique_object)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ContractError(f"invalid UTF-8 JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise ContractError("contract root must be an object")
    exact_keys(value, {"benchmark", "cases", "fuzz_seed", "schema"}, "root")
    if value["schema"] != "seen-byte-slices-v1":
        raise ContractError("schema must be seen-byte-slices-v1")
    bounded_int(value["fuzz_seed"], "fuzz_seed", 0, MAX_COUNT)
    cases = value["cases"]
    if not isinstance(cases, list) or not 1 <= len(cases) <= MAX_CASES:
        raise ContractError("cases must contain 1 through 128 entries")
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            raise ContractError(f"cases[{index}] must be an object")
        exact_keys(case, {"count", "expected", "offset", "total"}, f"cases[{index}]")
        total = bounded_int(case["total"], f"cases[{index}].total", -1, MAX_COUNT)
        offset = bounded_int(case["offset"], f"cases[{index}].offset", -1, MAX_COUNT)
        count = bounded_int(case["count"], f"cases[{index}].count", -1, MAX_COUNT)
        expected = case["expected"]
        if expected not in {"ok", "byte.invalid", "byte.limit"}:
            raise ContractError(f"cases[{index}].expected is not a stable byte code")
        if validate_range(total, offset, count) != expected:
            raise ContractError(f"cases[{index}] expectation does not match checked geometry")
    benchmark = value["benchmark"]
    if not isinstance(benchmark, dict):
        raise ContractError("benchmark must be an object")
    exact_keys(
        benchmark,
        {"baseline_ratio", "iterations_per_sample", "samples", "warmups"},
        "benchmark",
    )
    ratio = benchmark["baseline_ratio"]
    if isinstance(ratio, bool) or not isinstance(ratio, (int, float)) or ratio <= 0:
        raise ContractError("benchmark.baseline_ratio must be positive")
    bounded_int(benchmark["iterations_per_sample"], "benchmark.iterations_per_sample", 1000, 1_000_000)
    if bounded_int(benchmark["samples"], "benchmark.samples", 1, 100) != 30:
        raise ContractError("benchmark.samples must be 30")
    if bounded_int(benchmark["warmups"], "benchmark.warmups", 1, 20) != 5:
        raise ContractError("benchmark.warmups must be 5")
    return value


def fuzz_ranges(seconds: float, seed: int) -> int:
    if seconds <= 0 or seconds > 300:
        raise ContractError("fuzz duration must be in (0, 300]")
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    checked = 0
    while time.monotonic() < deadline:
        total = rng.randrange(-1, 1 << 34)
        offset = rng.randrange(-1, 1 << 34)
        count = rng.randrange(-1, 1 << 34)
        actual = validate_range(total, offset, count) == "ok"
        if actual != reference_valid(total, offset, count):
            raise ContractError(
                f"fuzz mismatch seed={seed} total={total} offset={offset} count={count}"
            )
        checked += 1
    return checked


def timed_batch(cases: list[tuple[int, int, int]], iterations: int, candidate: bool) -> int:
    started = time.perf_counter_ns()
    checksum = 0
    for index in range(iterations):
        total, offset, count = cases[index % len(cases)]
        if candidate:
            checksum += validate_range(total, offset, count) == "ok"
        else:
            checksum += reference_valid(total, offset, count)
    elapsed = time.perf_counter_ns() - started
    if checksum < 0:
        raise ContractError("unreachable benchmark checksum")
    return elapsed


def benchmark(contract: dict[str, object]) -> tuple[float, float, float]:
    config = contract["benchmark"]
    assert isinstance(config, dict)
    iterations = int(config["iterations_per_sample"])
    warmups = int(config["warmups"])
    samples = int(config["samples"])
    baseline_ratio = float(config["baseline_ratio"])
    cases = [
        (4, 1, 2),
        (4, 4, 0),
        (5_368_709_120, 4_294_967_296, 1_073_741_824),
        (4, 3, 2),
        (-1, 0, 0),
    ]
    for _ in range(warmups):
        timed_batch(cases, iterations, False)
        timed_batch(cases, iterations, True)
    ratios: list[float] = []
    for _ in range(samples):
        reference_ns = timed_batch(cases, iterations, False)
        candidate_ns = timed_batch(cases, iterations, True)
        ratios.append(candidate_ns / max(reference_ns, 1))
    median_ratio = statistics.median(ratios)
    limit = baseline_ratio * 1.05
    if median_ratio > limit:
        raise ContractError(
            f"hard 5% gate failed: median_ratio={median_ratio:.6f} limit={limit:.6f}"
        )
    return median_ratio, baseline_ratio, limit


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("contract", type=Path)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int)
    parser.add_argument("--benchmark", action="store_true")
    args = parser.parse_args()
    try:
        contract = load_contract(args.contract)
        print(f"byte-slices: valid {args.contract}")
        if args.fuzz_seconds:
            seed = args.seed if args.seed is not None else int(contract["fuzz_seed"])
            checked = fuzz_ranges(args.fuzz_seconds, seed)
            print(f"byte-slices: fuzz seed={seed} cases={checked} status=pass")
        if args.benchmark:
            ratio, baseline, limit = benchmark(contract)
            print(
                "byte-slices: benchmark warmups=5 samples=30 hard_gate=5% "
                f"median_ratio={ratio:.6f} baseline_ratio={baseline:.6f} "
                f"limit={limit:.6f} status=pass"
            )
    except (ContractError, OSError) as exc:
        print(f"byte-slices: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
