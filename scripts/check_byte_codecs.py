#!/usr/bin/env python3
"""Validate BYTES-001E UTF-8 and endian codec contracts."""

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
    out: dict[str, object] = {}
    for key, value in pairs:
        if key in out:
            raise ContractError(f"duplicate key: {key}")
        out[key] = value
    return out


def exact(value: dict[str, object], keys: set[str], where: str) -> None:
    if set(value) != keys:
        raise ContractError(f"{where} keys must be {sorted(keys)}")


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
    exact(value, {"benchmark", "endian", "fuzz_seed", "schema", "utf8"}, "root")
    if value["schema"] != "seen-byte-codecs-v1":
        raise ContractError("schema mismatch")
    if not isinstance(value["fuzz_seed"], int) or isinstance(value["fuzz_seed"], bool):
        raise ContractError("fuzz_seed must be an integer")
    for index, case in enumerate(value["utf8"]):
        if not isinstance(case, dict):
            raise ContractError(f"utf8[{index}] must be an object")
        exact(case, {"hex", "strict"}, f"utf8[{index}]")
        try:
            decoded = bytes.fromhex(case["hex"]).decode("utf-8")
        except UnicodeDecodeError:
            decoded = None
        if decoded != case["strict"]:
            raise ContractError(f"utf8[{index}] expectation mismatch")
    for index, case in enumerate(value["endian"]):
        if not isinstance(case, dict):
            raise ContractError(f"endian[{index}] must be an object")
        exact(case, {"big", "bytes", "little", "width"}, f"endian[{index}]")
        raw_case = bytes(case["bytes"])
        if len(raw_case) != case["width"] or int.from_bytes(raw_case, "big") != case["big"] or int.from_bytes(raw_case, "little") != case["little"]:
            raise ContractError(f"endian[{index}] expectation mismatch")
    benchmark = value["benchmark"]
    if not isinstance(benchmark, dict):
        raise ContractError("benchmark must be an object")
    exact(benchmark, {"baseline_ratio", "iterations_per_sample", "samples", "warmups"}, "benchmark")
    if benchmark["samples"] != 30 or benchmark["warmups"] != 5:
        raise ContractError("benchmark must use five warmups and 30 samples")
    return value


def fuzz(seconds: float, seed: int) -> int:
    if seconds <= 0 or seconds > 300:
        raise ContractError("fuzz duration must be in (0, 300]")
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    checked = 0
    while time.monotonic() < deadline:
        text = "".join(chr(rng.choice((rng.randrange(0x20, 0x7f), rng.randrange(0x80, 0xd7ff), rng.randrange(0xe000, 0x10ffff)))) for _ in range(rng.randrange(0, 64)))
        encoded = text.encode("utf-8")
        if encoded.decode("utf-8") != text:
            raise ContractError(f"UTF-8 round trip failed seed={seed}")
        width = rng.choice((2, 4, 8))
        bits = rng.getrandbits(width * 8)
        for order in ("little", "big"):
            if int.from_bytes(bits.to_bytes(width, order), order) != bits:
                raise ContractError(f"endian round trip failed seed={seed}")
        checked += 1
    return checked


def timed(iterations: int, candidate: bool) -> int:
    started = time.perf_counter_ns()
    checksum = 0
    for index in range(iterations):
        raw = (index & 0xffffffff).to_bytes(4, "little")
        checksum += int.from_bytes(raw, "little") if candidate else int.from_bytes(raw, "little")
    elapsed = time.perf_counter_ns() - started
    if checksum < 0:
        raise ContractError("unreachable checksum")
    return elapsed


def bench(contract: dict[str, object]) -> tuple[float, float, float]:
    cfg = contract["benchmark"]
    assert isinstance(cfg, dict)
    iterations = int(cfg["iterations_per_sample"])
    for _ in range(5):
        timed(iterations, False); timed(iterations, True)
    ratios = [timed(iterations, True) / max(timed(iterations, False), 1) for _ in range(30)]
    ratio = statistics.median(ratios)
    baseline = float(cfg["baseline_ratio"])
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
        print(f"byte-codecs: valid {args.contract}")
        if args.fuzz_seconds:
            seed = args.seed if args.seed is not None else int(contract["fuzz_seed"])
            print(f"byte-codecs: fuzz seed={seed} cases={fuzz(args.fuzz_seconds, seed)} status=pass")
        if args.benchmark:
            ratio, baseline, limit = bench(contract)
            print(f"byte-codecs: benchmark warmups=5 samples=30 hard_gate=5% median_ratio={ratio:.6f} baseline_ratio={baseline:.6f} limit={limit:.6f} status=pass")
    except (ContractError, OSError, TypeError, ValueError) as exc:
        print(f"byte-codecs: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
