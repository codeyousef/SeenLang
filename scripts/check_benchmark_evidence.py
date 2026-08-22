#!/usr/bin/env python3
"""Validate canonical TEST-002C Colibri benchmark evidence."""

import argparse
import json
import random
import sys
import time
from pathlib import Path

SCHEMA = "seen-benchmark-evidence-v1"
MAX_BYTES = 1_048_576
WARMUPS = 5
SAMPLES = 30
MAX_SAMPLE_NS = 1_000_000_000_000
MATURITY = {"unsupported", "compile-only", "experimental-hardware", "verified", "production-certified"}
FIELDS = {
    "backend", "bandwidth_bytes_per_second", "baseline_commit", "baseline_median_ns",
    "cgroup", "command", "correctness", "designated_kernel", "fallback_count",
    "hardware", "hardware_executed", "maturity", "max_regression_percent",
    "median_ns", "name", "peak_rss_bytes", "peak_vram_bytes", "samples_ns",
    "schema", "source_commit", "target", "toolchain", "transfer_bytes", "warmups",
}
CGROUP_FIELDS = {"jobs", "memory_max_bytes", "memory_swap_max_bytes", "opt_jobs", "pids_max"}


class ContractError(ValueError):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def fail(code, message):
    raise ContractError(code, message)


def pairs(items):
    output = {}
    for key, value in items:
        if key in output:
            fail("invalid", f"duplicate field: {key}")
        output[key] = value
    return output


def integer(value):
    return isinstance(value, int) and not isinstance(value, bool)


def text(value, maximum):
    return isinstance(value, str) and 0 < len(value) <= maximum and all(32 <= ord(c) <= 126 for c in value)


def commit(value):
    return isinstance(value, str) and len(value) == 40 and all(c in "0123456789abcdef" for c in value)


def validate(raw, max_bytes=MAX_BYTES, cancelled=False):
    if not integer(max_bytes) or not 1 <= max_bytes <= MAX_BYTES:
        fail("limit", "invalid benchmark evidence byte limit")
    if len(raw) > max_bytes:
        fail("limit", "benchmark evidence byte limit exceeded")
    try:
        value = json.loads(raw.decode(), object_pairs_hook=pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {type(error).__name__}")
    if cancelled:
        fail("cancelled", "benchmark evidence validation was cancelled")
    if not isinstance(value, dict) or set(value) != FIELDS:
        fail("invalid", "invalid benchmark evidence fields")
    if value["schema"] != SCHEMA:
        fail("invalid", "unsupported benchmark evidence schema")
    if value["target"] != "linux-x86_64":
        fail("platform", "unsupported benchmark target")
    if not text(value["name"], 128) or not text(value["backend"], 64):
        fail("invalid", "benchmark name or backend is invalid")
    if value["maturity"] not in MATURITY:
        fail("invalid", "benchmark maturity is invalid")
    if not text(value["hardware"], 256) or not text(value["toolchain"], 256):
        fail("invalid", "benchmark environment is invalid")
    if not text(value["command"], 4096):
        fail("invalid", "benchmark command is invalid")
    if not commit(value["source_commit"]) or not commit(value["baseline_commit"]):
        fail("invalid", "benchmark commit identity is invalid")
    cgroup = value["cgroup"]
    if not isinstance(cgroup, dict) or set(cgroup) != CGROUP_FIELDS:
        fail("invalid", "invalid cgroup evidence fields")
    if any(not integer(cgroup[key]) for key in CGROUP_FIELDS):
        fail("invalid", "cgroup limits must be integers")
    if cgroup["memory_max_bytes"] < 1 or cgroup["memory_swap_max_bytes"] != 0:
        fail("limit", "benchmark memory containment is invalid")
    if not 1 <= cgroup["pids_max"] <= 24:
        fail("limit", "benchmark task containment is invalid")
    if cgroup["jobs"] != 1 or cgroup["opt_jobs"] != 1:
        fail("limit", "benchmark worker containment is invalid")
    samples = value["samples_ns"]
    if value["warmups"] != WARMUPS or value["max_regression_percent"] != 5:
        fail("limit", "benchmark 5/30/5 policy is invalid")
    if not isinstance(samples, list) or len(samples) != SAMPLES:
        fail("limit", "benchmark sample count is invalid")
    if any(not integer(sample) for sample in samples):
        fail("invalid", "benchmark samples must be integers")
    if any(not 1 <= sample <= MAX_SAMPLE_NS for sample in samples):
        fail("limit", "benchmark sample is outside bounds")
    if samples != sorted(samples):
        fail("invalid", "benchmark samples are not canonical")
    median = (samples[14] + samples[15]) // 2
    numeric = ("median_ns", "baseline_median_ns", "bandwidth_bytes_per_second", "peak_rss_bytes", "peak_vram_bytes", "transfer_bytes", "fallback_count")
    if any(not integer(value[key]) for key in numeric):
        fail("invalid", "benchmark metrics must be integers")
    if value["median_ns"] != median:
        fail("invalid", "benchmark median does not match samples")
    if value["baseline_median_ns"] < 1 or value["bandwidth_bytes_per_second"] < 1:
        fail("limit", "benchmark baseline or bandwidth is invalid")
    if value["peak_rss_bytes"] < 1 or value["peak_vram_bytes"] < 0:
        fail("limit", "benchmark memory metrics are invalid")
    if value["transfer_bytes"] < 0 or value["fallback_count"] < 0:
        fail("invalid", "benchmark counters are invalid")
    if value["correctness"] is not True:
        fail("invalid", "benchmark correctness must pass")
    if not isinstance(value["designated_kernel"], bool):
        fail("invalid", "designated-kernel marker must be Boolean")
    if not isinstance(value["hardware_executed"], bool):
        fail("invalid", "hardware-executed marker must be Boolean")
    if value["designated_kernel"]:
        if value["fallback_count"] != 0 or value["hardware_executed"] is not True:
            fail("regression", "designated kernel used fallback or no hardware")
        if value["maturity"] in {"unsupported", "compile-only"}:
            fail("regression", "designated kernel lacks hardware maturity")
        if median * 100 > value["baseline_median_ns"] * 105:
            fail("regression", "designated kernel exceeded the hard five percent gate")
    if value["backend"] == "cpu":
        if value["peak_vram_bytes"] != 0 or value["transfer_bytes"] != 0:
            fail("invalid", "CPU evidence cannot claim GPU resources")
        if value["hardware_executed"]:
            fail("invalid", "CPU evidence cannot claim GPU execution")
    return value


def canonical(value):
    return (json.dumps(validate(json.dumps(value).encode()), separators=(",", ":"), sort_keys=True) + "\n").encode()


def fuzz(raw, seconds, seed):
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = rejected = 0
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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--max-bytes", type=int, default=MAX_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        metadata = args.evidence.lstat()
        if args.evidence.is_symlink() or not args.evidence.is_file():
            fail("limit", "benchmark evidence file is unsafe")
        if metadata.st_size > args.max_bytes:
            fail("limit", "benchmark evidence file is oversized")
        raw = args.evidence.read_bytes()
        value = validate(raw, args.max_bytes, args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical(value))
    except ContractError as error:
        print(f"test.002c.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"test.002c.io: {type(error).__name__}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
