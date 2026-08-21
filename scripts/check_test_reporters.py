#!/usr/bin/env python3
"""Strict host oracle for Seen human, JSON, and JUnit test reports."""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

FORMATS = {"human", "json", "junit"}
MAX_BYTES = 16 * 1024 * 1024


class ContractError(ValueError):
    pass


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result = {}
    for key, value in pairs:
        if key in result:
            raise ContractError("test.001e.invalid: duplicate JSON field")
        result[key] = value
    return result


def read(path: Path) -> bytes:
    try:
        if not 1 <= path.stat().st_size <= MAX_BYTES:
            raise ContractError("test.001e.limit: report byte limit exceeded")
        return path.read_bytes()
    except ContractError:
        raise
    except OSError as exc:
        raise ContractError("test.001e.io: report is unavailable") from exc


def validate_json(raw: bytes) -> tuple[int, int, int]:
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=no_duplicates)
    except ContractError:
        raise
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise ContractError("test.001e.invalid: JSON report is invalid") from exc
    keys = {"schema", "results", "passed", "failed", "skipped", "exit_code"}
    if not isinstance(value, dict) or set(value) != keys or value["schema"] != "seen-test-run-v1":
        raise ContractError("test.001e.invalid: JSON envelope is invalid")
    if not isinstance(value["results"], list):
        raise ContractError("test.001e.invalid: JSON results are invalid")
    counts = {"passed": 0, "failed": 0, "skipped": 0}
    previous = ""
    for result in value["results"]:
        if not isinstance(result, dict) or set(result) != {"path", "status", "exit_code"}:
            raise ContractError("test.001e.invalid: JSON result is invalid")
        path, status, code = result["path"], result["status"], result["exit_code"]
        if (not isinstance(path, str) or not path or path <= previous or
                status not in counts or type(code) is not int or not 0 <= code <= 255 or
                ((status in {"passed", "skipped"}) != (code == 0))):
            raise ContractError("test.001e.invalid: JSON result is inconsistent")
        counts[status] += 1
        previous = path
    if any(type(value[key]) is not int or value[key] != counts[key] for key in counts):
        raise ContractError("test.001e.invalid: JSON counters are inconsistent")
    if value["exit_code"] != (1 if counts["failed"] else 0):
        raise ContractError("test.001e.invalid: JSON exit code is inconsistent")
    return counts["passed"], counts["failed"], counts["skipped"]


def validate_human(raw: bytes) -> tuple[int, int, int]:
    try:
        text = raw.decode("utf-8")
    except UnicodeError as exc:
        raise ContractError("test.001e.invalid: human report is not UTF-8") from exc
    if not text.endswith("\n"):
        raise ContractError("test.001e.invalid: human report lacks final newline")
    lines = text.splitlines()
    if not lines or not lines[-1].startswith("test result: "):
        raise ContractError("test.001e.invalid: human summary is missing")
    counts = [0, 0, 0]
    previous = ""
    for line in lines[:-1]:
        if line.startswith("PASS "):
            path, index = line[5:], 0
        elif line.startswith("SKIP "):
            path, index = line[5:], 2
        elif line.startswith("FAIL ") and " (exit " in line and line.endswith(")"):
            marker = line.rfind(" (exit ")
            path, index = line[5:marker], 1
            try:
                code = int(line[marker + 7:-1])
            except ValueError as exc:
                raise ContractError("test.001e.invalid: human exit code is invalid") from exc
            if not 1 <= code <= 255:
                raise ContractError("test.001e.invalid: human exit code is invalid")
        else:
            raise ContractError("test.001e.invalid: human result is invalid")
        if not path or path <= previous:
            raise ContractError("test.001e.invalid: human order is invalid")
        counts[index] += 1
        previous = path
    expected = f"test result: {counts[0]} passed; {counts[1]} failed; {counts[2]} skipped"
    if lines[-1] != expected:
        raise ContractError("test.001e.invalid: human counters are inconsistent")
    return tuple(counts)


def validate_junit(raw: bytes) -> tuple[int, int, int]:
    if b"<!DOCTYPE" in raw or b"<!ENTITY" in raw:
        raise ContractError("test.001e.invalid: JUnit declarations are forbidden")
    try:
        root = ET.fromstring(raw)
    except (ET.ParseError, LookupError, ValueError) as exc:
        raise ContractError("test.001e.invalid: JUnit XML is invalid") from exc
    if root.tag != "testsuite" or set(root.attrib) != {"name", "tests", "failures", "skipped"}:
        raise ContractError("test.001e.invalid: JUnit suite is invalid")
    if not root.attrib["name"]:
        raise ContractError("test.001e.invalid: JUnit suite name is empty")
    counts = [0, 0, 0]
    previous = ""
    for case in list(root):
        if case.tag != "testcase" or set(case.attrib) != {"name"} or case.attrib["name"] <= previous:
            raise ContractError("test.001e.invalid: JUnit testcase is invalid")
        children = list(case)
        if not children:
            counts[0] += 1
        elif len(children) == 1 and children[0].tag == "failure" and set(children[0].attrib) == {"message"}:
            message = children[0].attrib["message"]
            if not message.startswith("exit ") or not message[5:].isdigit() or not 1 <= int(message[5:]) <= 255:
                raise ContractError("test.001e.invalid: JUnit failure code is invalid")
            counts[1] += 1
        elif len(children) == 1 and children[0].tag == "skipped" and not children[0].attrib:
            counts[2] += 1
        else:
            raise ContractError("test.001e.invalid: JUnit testcase state is invalid")
        previous = case.attrib["name"]
    try:
        declared = tuple(int(root.attrib[key]) for key in ("tests", "failures", "skipped"))
    except ValueError as exc:
        raise ContractError("test.001e.invalid: JUnit counters are invalid") from exc
    if declared != (sum(counts), counts[1], counts[2]):
        raise ContractError("test.001e.invalid: JUnit counters are inconsistent")
    return tuple(counts)


def validate(raw: bytes, format_name: str) -> tuple[int, int, int]:
    if format_name == "human":
        return validate_human(raw)
    if format_name == "json":
        return validate_json(raw)
    if format_name == "junit":
        return validate_junit(raw)
    raise ContractError("test.001e.invalid: reporter format is invalid")


def fuzz(raw: bytes, format_name: str, seconds: float, seed: int) -> int:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = rejected = 0
    while time.monotonic() < deadline or cases == 0:
        mutated = bytearray(raw)
        mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            validate(bytes(mutated), format_name)
        except ContractError:
            rejected += 1
        cases += 1
    if rejected == 0:
        raise ContractError("test.001e.invalid: fuzz produced no rejection")
    print(f"seed={seed} cases={cases} rejected={rejected}", file=sys.stderr)
    return cases


def benchmark(raw: bytes, format_name: str, limit_ms: float) -> None:
    for _ in range(5):
        validate(raw, format_name)
    samples = []
    for _ in range(30):
        start = time.perf_counter_ns()
        validate(raw, format_name)
        samples.append((time.perf_counter_ns() - start) / 1_000_000)
    measured = sorted(samples)[len(samples) // 2]
    if measured > limit_ms * 1.05:
        raise ContractError("test.001e.limit: benchmark exceeded hard 5% gate")
    print(f"warmups=5 samples=30 median_ms={measured:.6f} status=pass")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate", type=Path, required=True)
    parser.add_argument("--format", choices=sorted(FORMATS), required=True)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--benchmark-limit-ms", type=float)
    args = parser.parse_args()
    if args.fuzz_seconds < 0:
        raise ContractError("test.001e.invalid: negative fuzz duration")
    raw = read(args.validate)
    validate(raw, args.format)
    if args.fuzz_seconds:
        fuzz(raw, args.format, args.fuzz_seconds, args.seed)
    if args.benchmark_limit_ms is not None:
        if args.benchmark_limit_ms <= 0:
            raise ContractError("test.001e.invalid: benchmark limit is invalid")
        benchmark(raw, args.format, args.benchmark_limit_ms)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ContractError as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(2)
