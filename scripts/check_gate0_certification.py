#!/usr/bin/env python3
"""Validate or generate canonical P0-GATE0-001 clean-checkout evidence."""

from __future__ import annotations

import argparse
import json
import os
import random
import stat
import sys
import time
from pathlib import Path

SCHEMA = "seen-gate0-certification-v1"
TARGET = "linux-x86_64"
STEPS = ("build", "test", "fuzz-smoke", "package")
PLATFORMS = (
    ("linux-x86_64", "required"),
    ("linux-arm64", "static-policy"),
    ("macos", "unsupported-fail-closed"),
    ("windows", "unsupported-fail-closed"),
)
MAX_BYTES = 1_048_576
FIELDS = {
    "active_ci", "clean_checkout", "compiler", "disabled_workflow_count",
    "limits", "platforms", "repair_allowed", "schema", "source_commit",
    "steps", "target",
}
LIMIT_FIELDS = {
    "jobs", "memory_max_bytes", "memory_swap_max_bytes", "opt_jobs",
    "pids_max", "timeout_seconds",
}
COMPILER_FIELDS = {"compatibility_sha256", "path", "pinned", "sha256"}
STEP_FIELDS = {"artifact_sha256", "name", "status"}
PLATFORM_FIELDS = {"name", "support"}


class ContractError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise ContractError(code, message)


def pairs(items: list[tuple[str, object]]) -> dict[str, object]:
    output: dict[str, object] = {}
    for key, value in items:
        if key in output:
            fail("invalid", f"duplicate field: {key}")
        output[key] = value
    return output


def integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def digest(value: object, size: int) -> bool:
    return (
        isinstance(value, str)
        and len(value) == size
        and all(character in "0123456789abcdef" for character in value)
    )


def safe_path(value: object) -> bool:
    if not isinstance(value, str) or not 0 < len(value) <= 4096:
        return False
    if value.startswith(('/', '-')) or '\\' in value or '//' in value:
        return False
    return all(part not in ('', '.', '..') for part in value.split('/'))


def validate(raw: bytes, max_bytes: int = MAX_BYTES, cancelled: bool = False) -> dict[str, object]:
    if not integer(max_bytes) or not 1 <= max_bytes <= MAX_BYTES:
        fail("limit", "invalid evidence byte limit")
    if len(raw) > max_bytes:
        fail("limit", "Gate 0 evidence byte limit exceeded")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {type(error).__name__}")
    if cancelled:
        fail("cancelled", "Gate 0 certification was cancelled")
    if not isinstance(value, dict) or set(value) != FIELDS:
        fail("invalid", "Gate 0 evidence fields are invalid")
    if value["target"] != TARGET:
        fail("platform", "Gate 0 certification target is unsupported")
    if value["schema"] != SCHEMA or not digest(value["source_commit"], 40):
        fail("invalid", "Gate 0 evidence identity is invalid")
    if value["clean_checkout"] is not True or value["active_ci"] is not True:
        fail("unverified", "clean-checkout active-CI evidence is required")
    if value["repair_allowed"] is not False or value["disabled_workflow_count"] != 0:
        fail("invalid", "repair and disabled workflows are forbidden")

    limits = value["limits"]
    if not isinstance(limits, dict) or set(limits) != LIMIT_FIELDS:
        fail("invalid", "Gate 0 limit fields are invalid")
    if any(not integer(limits[key]) for key in LIMIT_FIELDS):
        fail("invalid", "Gate 0 limits must be integers")
    if limits["memory_max_bytes"] < 1 or limits["memory_swap_max_bytes"] != 0:
        fail("limit", "memory containment is invalid")
    if not 1 <= limits["pids_max"] <= 24 or limits["jobs"] != 1 or limits["opt_jobs"] != 1:
        fail("limit", "task or worker containment is invalid")
    if not 1 <= limits["timeout_seconds"] <= 3600:
        fail("limit", "certification timeout is invalid")

    compiler = value["compiler"]
    if not isinstance(compiler, dict) or set(compiler) != COMPILER_FIELDS:
        fail("invalid", "compiler pin fields are invalid")
    if compiler["pinned"] is not True or not safe_path(compiler["path"]):
        fail("unverified", "compiler path is not a pinned repository artifact")
    if not digest(compiler["sha256"], 64) or not digest(compiler["compatibility_sha256"], 64):
        fail("unverified", "compiler pin digests are invalid")

    steps = value["steps"]
    if not isinstance(steps, list) or len(steps) != len(STEPS):
        fail("limit", "exactly four certification steps are required")
    for index, step in enumerate(steps):
        if not isinstance(step, dict) or set(step) != STEP_FIELDS:
            fail("invalid", "certification step fields are invalid")
        if step["name"] != STEPS[index] or step["status"] != "passed":
            fail("unverified", "certification step order or status is invalid")
        if not digest(step["artifact_sha256"], 64):
            fail("unverified", "certification step artifact digest is invalid")

    platforms = value["platforms"]
    if not isinstance(platforms, list) or len(platforms) != len(PLATFORMS):
        fail("platform", "platform applicability is incomplete")
    for index, platform in enumerate(platforms):
        if not isinstance(platform, dict) or set(platform) != PLATFORM_FIELDS:
            fail("invalid", "platform applicability fields are invalid")
        if (platform["name"], platform["support"]) != PLATFORMS[index]:
            fail("platform", "platform applicability is not canonical")
    return value


def canonical(value: dict[str, object]) -> bytes:
    raw = (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8")
    validate(raw)
    return raw


def build(args: argparse.Namespace) -> dict[str, object]:
    step_digests = (args.build_sha256, args.test_sha256, args.fuzz_sha256, args.package_sha256)
    value = {
        "active_ci": True,
        "clean_checkout": True,
        "compiler": {
            "compatibility_sha256": args.compatibility_sha256,
            "path": "bootstrap/stage1_frozen",
            "pinned": True,
            "sha256": args.compiler_sha256,
        },
        "disabled_workflow_count": 0,
        "limits": {
            "jobs": 1,
            "memory_max_bytes": args.memory_max_bytes,
            "memory_swap_max_bytes": 0,
            "opt_jobs": 1,
            "pids_max": args.pids_max,
            "timeout_seconds": args.timeout_seconds,
        },
        "platforms": [{"name": name, "support": support} for name, support in PLATFORMS],
        "repair_allowed": False,
        "schema": SCHEMA,
        "source_commit": args.source_commit,
        "steps": [
            {"artifact_sha256": step_digests[index], "name": name, "status": "passed"}
            for index, name in enumerate(STEPS)
        ],
        "target": TARGET,
    }
    return validate(canonical(value))


def atomic_write(path: Path, payload: bytes, cancelled: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.parent / f".{path.name}.tmp.{os.getpid()}"
    try:
        with temporary.open("xb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        if cancelled:
            fail("cancelled", "Gate 0 certification was cancelled")
        os.replace(temporary, path)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def safe_read(path: Path, maximum: int) -> bytes:
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        fail("invalid", "evidence must be a regular non-symlink file")
    if metadata.st_size > maximum:
        fail("limit", "Gate 0 evidence file is oversized")
    return path.read_bytes()


def fuzz(raw: bytes, seconds: float, seed: int) -> tuple[int, int]:
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


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--evidence", type=Path)
    result.add_argument("--output", type=Path)
    result.add_argument("--source-commit")
    result.add_argument("--compiler-sha256")
    result.add_argument("--compatibility-sha256")
    result.add_argument("--build-sha256")
    result.add_argument("--test-sha256")
    result.add_argument("--fuzz-sha256")
    result.add_argument("--package-sha256")
    result.add_argument("--memory-max-bytes", type=int)
    result.add_argument("--pids-max", type=int, default=24)
    result.add_argument("--timeout-seconds", type=int, default=1800)
    result.add_argument("--max-bytes", type=int, default=MAX_BYTES)
    result.add_argument("--fuzz-seconds", type=float, default=0)
    result.add_argument("--seed", type=int, default=1101)
    result.add_argument("--test-cancel-after-read", action="store_true")
    result.add_argument("--test-cancel-before-commit", action="store_true")
    return result


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        if args.evidence:
            if args.output or args.source_commit:
                fail("invalid", "validation and generation modes are exclusive")
            raw = safe_read(args.evidence, args.max_bytes)
            value = validate(raw, args.max_bytes, args.test_cancel_after_read)
        else:
            required = (
                args.output, args.source_commit, args.compiler_sha256,
                args.compatibility_sha256, args.build_sha256, args.test_sha256,
                args.fuzz_sha256, args.package_sha256, args.memory_max_bytes,
            )
            if any(item is None for item in required):
                fail("invalid", "generation requires output, source, compiler, step, and memory pins")
            value = build(args)
            raw = canonical(value)
            atomic_write(args.output, raw, args.test_cancel_before_commit)
        if args.fuzz_seconds:
            cases, rejected = fuzz(canonical(value), args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical(value))
    except ContractError as error:
        print(f"p0.gate0.001.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"p0.gate0.001.io: {type(error).__name__}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
