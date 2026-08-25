#!/usr/bin/env python3
"""Validate BYTES-001D bounded copy/adaptor progress contracts."""

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


def copy_progress(source: int, destination: int, limit: int, scratch: int) -> tuple[int, int]:
    read = 0
    written = 0
    while read < limit and read < source:
        chunk = min(scratch, limit - read, source - read)
        read += chunk
        accepted = min(chunk, destination - written)
        written += accepted
        if accepted < chunk:
            break
    return read, written


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
        {"benchmark", "cases", "fuzz_seed", "schema", "scratch_capacity"},
        "root",
    )
    if value["schema"] != "seen-byte-adapters-v1":
        raise ContractError("schema must be seen-byte-adapters-v1")
    scratch = bounded_int(value["scratch_capacity"], "scratch_capacity", 1, 1 << 20)
    bounded_int(value["fuzz_seed"], "fuzz_seed", 0, (1 << 63) - 1)
    cases = value["cases"]
    if not isinstance(cases, list) or not 1 <= len(cases) <= MAX_CASES:
        raise ContractError("cases must contain 1 through 128 entries")
    expected_keys = {"destination", "expected_read", "expected_written", "limit", "source"}
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            raise ContractError(f"cases[{index}] must be an object")
        exact_keys(case, expected_keys, f"cases[{index}]")
        source = bounded_int(case["source"], f"cases[{index}].source", 0, 1 << 30)
        destination = bounded_int(case["destination"], f"cases[{index}].destination", 0, 1 << 30)
        limit = bounded_int(case["limit"], f"cases[{index}].limit", 0, 1 << 30)
        expected = copy_progress(source, destination, limit, scratch)
        if expected != (case["expected_read"], case["expected_written"]):
            raise ContractError(f"cases[{index}] progress mismatch")
    benchmark = value["benchmark"]
    if not isinstance(benchmark, dict):
        raise ContractError("benchmark must be an object")
    exact_keys(benchmark, {"baseline_ratio", "iterations_per_sample", "samples", "warmups"}, "benchmark")
    ratio = benchmark["baseline_ratio"]
    if isinstance(ratio, bool) or not isinstance(ratio, (int, float)) or ratio <= 0:
        raise ContractError("benchmark.baseline_ratio must be positive")
    bounded_int(benchmark["iterations_per_sample"], "benchmark.iterations_per_sample", 1000, 1_000_000)
    if bounded_int(benchmark["samples"], "benchmark.samples", 1, 100) != 30:
        raise ContractError("benchmark.samples must be 30")
    if bounded_int(benchmark["warmups"], "benchmark.warmups", 1, 20) != 5:
        raise ContractError("benchmark.warmups must be 5")
    return value


def fuzz_adapters(seconds: float, seed: int) -> int:
    if seconds <= 0 or seconds > 300:
        raise ContractError("fuzz duration must be in (0, 300]")
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    checked = 0
    while time.monotonic() < deadline:
        source = rng.randrange(0, 8192)
        destination = rng.randrange(0, 8192)
        limit = rng.randrange(0, 8192)
        scratch = rng.randrange(1, 257)
        read, written = copy_progress(source, destination, limit, scratch)
        if not (0 <= written <= read <= min(source, limit)):
            raise ContractError(f"copy invariant failed seed={seed}")
        checked += 1
    return checked


def timed_copy(iterations: int, candidate: bool) -> int:
    started = time.perf_counter_ns()
    checksum = 0
    for index in range(iterations):
        source = (index & 255) + 1
        if candidate:
            read, written = copy_progress(source, source, source, 64)
            checksum += read + written
        else:
            read = 0
            written = 0
            while read < source:
                chunk = min(64, source - read)
                read += chunk
                written += chunk
            checksum += read + written
    elapsed = time.perf_counter_ns() - started
    if checksum < 0:
        raise ContractError("unreachable benchmark checksum")
    return elapsed


def benchmark(contract: dict[str, object]) -> tuple[float, float, float]:
    config = contract["benchmark"]
    assert isinstance(config, dict)
    iterations = int(config["iterations_per_sample"])
    for _ in range(int(config["warmups"])):
        timed_copy(iterations, False)
        timed_copy(iterations, True)
    ratios = []
    for _ in range(int(config["samples"])):
        baseline_ns = timed_copy(iterations, False)
        candidate_ns = timed_copy(iterations, True)
        ratios.append(candidate_ns / max(baseline_ns, 1))
    median_ratio = statistics.median(ratios)
    baseline_ratio = float(config["baseline_ratio"])
    limit = baseline_ratio * 1.05
    if median_ratio > limit:
        raise ContractError(f"hard 5% gate failed: median_ratio={median_ratio:.6f} limit={limit:.6f}")
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
        print(f"byte-adapters: valid {args.contract}")
        if args.fuzz_seconds:
            seed = args.seed if args.seed is not None else int(contract["fuzz_seed"])
            checked = fuzz_adapters(args.fuzz_seconds, seed)
            print(f"byte-adapters: fuzz seed={seed} cases={checked} status=pass")
        if args.benchmark:
            ratio, baseline, limit = benchmark(contract)
            print(
                "byte-adapters: benchmark warmups=5 samples=30 hard_gate=5% "
                f"median_ratio={ratio:.6f} baseline_ratio={baseline:.6f} limit={limit:.6f} status=pass"
            )
    except (ContractError, OSError) as exc:
        print(f"byte-adapters: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
