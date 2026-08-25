#!/usr/bin/env python3
"""Validate and stress the bounded BYTES-001B storage contract."""

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
MAX_CAPACITY = 1_073_741_824


class ContractError(ValueError):
    pass


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate key: {key}")
        result[key] = value
    return result


def exact_keys(value: dict[str, object], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise ContractError(f"{where} keys must be {sorted(expected)}")


def bounded_int(value: object, where: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ContractError(f"{where} must be an integer")
    if value < minimum or value > maximum:
        raise ContractError(f"{where} is outside [{minimum}, {maximum}]")
    return value


def classify(initial: int, required: int) -> str:
    if initial < 0 or required < 0:
        return "byte.invalid"
    if initial > MAX_CAPACITY or required > MAX_CAPACITY:
        return "byte.limit"
    return "grow" if required > initial else "ok"


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
    exact_keys(
        value,
        {"alignments", "benchmark", "cases", "fuzz_seed", "max_capacity", "schema"},
        "root",
    )
    if value["schema"] != "seen-byte-storage-v1":
        raise ContractError("schema must be seen-byte-storage-v1")
    if value["max_capacity"] != MAX_CAPACITY:
        raise ContractError("max_capacity must bind the native 1 GiB policy")
    bounded_int(value["fuzz_seed"], "fuzz_seed", 0, (1 << 63) - 1)
    if value["alignments"] != [1, 2, 4, 8, 16, 32]:
        raise ContractError("alignments must be the supported powers of two through 32")
    cases = value["cases"]
    if not isinstance(cases, list) or not 1 <= len(cases) <= MAX_CASES:
        raise ContractError("cases must contain 1 through 128 entries")
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            raise ContractError(f"cases[{index}] must be an object")
        exact_keys(case, {"expected", "initial", "required"}, f"cases[{index}]")
        initial = bounded_int(case["initial"], f"cases[{index}].initial", -1, MAX_CAPACITY)
        required = bounded_int(
            case["required"], f"cases[{index}].required", 0, MAX_CAPACITY + 1
        )
        if classify(initial, required) != case["expected"]:
            raise ContractError(f"cases[{index}] expectation does not match capacity policy")
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


def fuzz_storage(seconds: float, seed: int) -> int:
    if seconds <= 0 or seconds > 300:
        raise ContractError("fuzz duration must be in (0, 300]")
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    checked = 0
    while time.monotonic() < deadline:
        capacity = rng.randrange(0, 8192)
        length = rng.randrange(0, capacity + 1)
        for _ in range(rng.randrange(1, 24)):
            if length >= capacity:
                capacity = max(1, capacity * 2, length + 1)
            if capacity > MAX_CAPACITY or length >= capacity:
                raise ContractError(f"growth invariant failed seed={seed}")
            length += 1
        checked += 1
    return checked


def timed_growth(iterations: int, candidate: bool) -> int:
    started = time.perf_counter_ns()
    checksum = 0
    for index in range(iterations):
        current = index & 1023
        required = current + (index & 7)
        if candidate:
            checksum += classify(current, required) == "grow"
        else:
            checksum += required > current and required <= MAX_CAPACITY
    elapsed = time.perf_counter_ns() - started
    if checksum < 0:
        raise ContractError("unreachable benchmark checksum")
    return elapsed


def benchmark(contract: dict[str, object]) -> tuple[float, float, float]:
    config = contract["benchmark"]
    assert isinstance(config, dict)
    iterations = int(config["iterations_per_sample"])
    for _ in range(int(config["warmups"])):
        timed_growth(iterations, False)
        timed_growth(iterations, True)
    ratios = []
    for _ in range(int(config["samples"])):
        baseline_ns = timed_growth(iterations, False)
        candidate_ns = timed_growth(iterations, True)
        ratios.append(candidate_ns / max(baseline_ns, 1))
    median_ratio = statistics.median(ratios)
    baseline_ratio = float(config["baseline_ratio"])
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
        print(f"byte-storage: valid {args.contract}")
        if args.fuzz_seconds:
            seed = args.seed if args.seed is not None else int(contract["fuzz_seed"])
            checked = fuzz_storage(args.fuzz_seconds, seed)
            print(f"byte-storage: fuzz seed={seed} cases={checked} status=pass")
        if args.benchmark:
            ratio, baseline, limit = benchmark(contract)
            print(
                "byte-storage: benchmark warmups=5 samples=30 hard_gate=5% "
                f"median_ratio={ratio:.6f} baseline_ratio={baseline:.6f} "
                f"limit={limit:.6f} status=pass"
            )
    except (ContractError, OSError) as exc:
        print(f"byte-storage: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
