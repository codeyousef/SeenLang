#!/usr/bin/env python3
"""Validate the deterministic required-CI containment contract."""

from __future__ import annotations

import argparse
import json
import os
import signal
import sys
from pathlib import Path

MAX_BYTES_HARD = 1024 * 1024
EXPECTED = {
    "version": 1,
    "entrypoints": {
        "guard": "scripts/memory_guard.sh",
        "inner_gate": "scripts/ci_required.sh",
        "outer_gate": "scripts/run_ci_required.sh",
        "scope": "scripts/run_in_hard_memory_scope.sh",
    },
    "memory": {
        "available_divisor": 2,
        "memory_high_percent": 99,
        "swap_max_bytes": 0,
        "total_divisor": 4,
    },
    "platforms": {
        "linux-aarch64": "static-policy",
        "linux-x86_64": "required",
        "macos": "unsupported-fail-closed",
        "windows": "unsupported-fail-closed",
    },
    "process": {
        "allocation_limit": "aggregate-cap",
        "main_vmem_limit": "aggregate-cap",
        "optimizer_vmem_max_kib": 2097152,
        "timeout_seconds": 10800,
    },
    "tasks": {"hard_max": 24, "jobs": 1, "optimizer_jobs": 1},
}


class ContractError(ValueError):
    pass


def fail(code: str, message: str) -> None:
    raise ContractError(f"core.001b.{code}: {message}")


def positive_bounded(value: str, maximum: int, name: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise argparse.ArgumentTypeError(f"{name} must be an integer") from error
    if parsed < 1 or parsed > maximum:
        raise argparse.ArgumentTypeError(f"{name} must be between 1 and {maximum}")
    return parsed


def validate(document: object) -> dict[str, object]:
    if document != EXPECTED:
        fail("invalid", "containment policy differs from the reviewed contract")
    return EXPECTED


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("contract", type=Path)
    parser.add_argument("--max-bytes", default=str(64 * 1024))
    parser.add_argument("--test-cancel-after-read", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        max_bytes = positive_bounded(args.max_bytes, MAX_BYTES_HARD, "max-bytes")
        requested_path = args.contract
        if requested_path.is_symlink():
            fail("invalid", "contract must be a regular non-symlink file")
        path = requested_path.resolve(strict=True)
        if not path.is_file():
            fail("invalid", "contract must be a regular non-symlink file")
        raw = path.read_bytes()
        if len(raw) > max_bytes:
            fail("limit", f"contract byte limit exceeded ({max_bytes})")
        if raw.startswith(b"\xef\xbb\xbf"):
            fail("invalid", "contract must not contain a UTF-8 BOM")
        if args.test_cancel_after_read:
            if os.environ.get("SEEN_CI_CONTAINMENT_TEST_HOOKS") != "1":
                fail("invalid", "cancellation test hook is disabled")
            raise KeyboardInterrupt
        report = validate(json.loads(raw))
        sys.stdout.buffer.write(
            (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8")
        )
    except KeyboardInterrupt:
        print("ci-containment: core.001b.cancelled: validation cancelled", file=sys.stderr)
        return 130
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, ContractError, argparse.ArgumentTypeError) as error:
        print(f"ci-containment: {error}", file=sys.stderr)
        return 1
    return 0


def cancel(_signum: int, _frame: object) -> None:
    raise KeyboardInterrupt


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, cancel)
    sys.exit(main())
