#!/usr/bin/env python3
"""Strict host oracle for deterministic isolated test fixtures."""

from __future__ import annotations

import argparse
import json
import random
import shutil
import sys
import time
from pathlib import Path

SCHEMA = "seen-test-fixture-v1"
TOP_KEYS = {"schema", "name", "seed", "target", "files", "environment"}
FILE_KEYS = {"path", "content"}
ENV_KEYS = {"name", "value"}
TARGETS = {"linux-x86_64", "linux-arm64", "macos", "windows"}
MAX_FILES = 256
MAX_ENVIRONMENT = 64
MAX_PATH_BYTES = 4096
MAX_CONTENT_BYTES = 1_048_576
MAX_ENVIRONMENT_VALUE_BYTES = 4096
MAX_DOCUMENT_BYTES = 2 * 1_048_576
_WORKSPACE_TOKEN = object()


class ContractError(ValueError):
    pass


class FixtureWorkspace:
    def __init__(self, base: Path, root: Path, token: object) -> None:
        if token is not _WORKSPACE_TOKEN:
            raise ContractError("test.001d.invalid: invalid workspace owner")
        self.base = base
        self.root = root
        self._token = token
        self.cleaned = False


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError("test.001d.invalid: duplicate fixture field")
        result[key] = value
    return result


def safe_name(value: object) -> bool:
    return (isinstance(value, str) and value != "" and
            not value.startswith(".") and
            len(value.encode()) <= 128 and
            all(ch.isascii() and (ch.isalnum() or ch in "-._")
                for ch in value))


def safe_path(value: object) -> bool:
    return (isinstance(value, str) and value != "" and value.isascii() and
            len(value.encode()) <= MAX_PATH_BYTES and
            not value.startswith("/") and not value.endswith("/") and
            not value.startswith(".") and "/." not in value and
            ".." not in value and "//" not in value and "\\" not in value and
            all(ch.isalnum() or ch in "-._/" for ch in value))


def safe_environment_name(value: object) -> bool:
    if not isinstance(value, str) or not 1 <= len(value) <= 64:
        return False
    if value not in {"LANG", "LC_ALL", "TZ"} and not value.startswith("SEEN_TEST_"):
        return False
    if any(marker in value for marker in ("SECRET", "TOKEN", "PASSWORD")):
        return False
    return all(("A" <= ch <= "Z") or ch == "_" or
               (index > 0 and ch.isdigit()) for index, ch in enumerate(value))


def validate(payload: object) -> dict[str, object]:
    if not isinstance(payload, dict) or set(payload) != TOP_KEYS:
        raise ContractError("test.001d.invalid: fixture fields are not canonical")
    if payload["schema"] != SCHEMA or not safe_name(payload["name"]):
        raise ContractError("test.001d.invalid: fixture identity is invalid")
    if type(payload["seed"]) is not int or not 0 <= payload["seed"] <= 2_147_483_647:
        raise ContractError("test.001d.invalid: fixture seed is invalid")
    if payload["target"] not in TARGETS:
        raise ContractError("test.001d.platform: fixture target is unsupported")
    files = payload["files"]
    environment = payload["environment"]
    if not isinstance(files, list) or len(files) > MAX_FILES:
        raise ContractError("test.001d.limit: fixture file limit exceeded")
    if not isinstance(environment, list) or len(environment) > MAX_ENVIRONMENT:
        raise ContractError("test.001d.limit: fixture environment limit exceeded")
    previous = ""
    total = 0
    for item in files:
        if not isinstance(item, dict) or set(item) != FILE_KEYS:
            raise ContractError("test.001d.invalid: fixture file fields are invalid")
        path = item["path"]
        content = item["content"]
        if not safe_path(path) or (previous and path.encode() <= previous.encode()):
            raise ContractError("test.001d.invalid: fixture paths are not ordered")
        if not isinstance(content, str):
            raise ContractError("test.001d.invalid: fixture content is not text")
        total += len(content.encode())
        if total > MAX_CONTENT_BYTES:
            raise ContractError("test.001d.limit: fixture content limit exceeded")
        previous = path
    previous = ""
    for item in environment:
        if not isinstance(item, dict) or set(item) != ENV_KEYS:
            raise ContractError("test.001d.invalid: environment fields are invalid")
        name = item["name"]
        value = item["value"]
        if not safe_environment_name(name) or (previous and name <= previous):
            raise ContractError("test.001d.invalid: environment is not ordered")
        if (not isinstance(value, str) or
                len(value.encode()) > MAX_ENVIRONMENT_VALUE_BYTES or
                any(ord(ch) < 32 or ord(ch) > 126 for ch in value)):
            raise ContractError("test.001d.invalid: environment value is invalid")
        previous = name
    return payload


def load(path: Path) -> object:
    try:
        if not 1 <= path.stat().st_size <= MAX_DOCUMENT_BYTES:
            raise ContractError("test.001d.limit: fixture document limit exceeded")
        return json.loads(path.read_bytes().decode("utf-8"),
                          object_pairs_hook=no_duplicates)
    except ContractError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ContractError("test.001d.io: fixture input is unavailable") from exc


def materialize(payload: object, base: Path) -> FixtureWorkspace:
    fixture = validate(payload)
    if not base.is_absolute() or base.is_symlink() or not base.is_dir():
        raise ContractError("test.001d.invalid: fixture base is not physical")
    physical = base.resolve(strict=True)
    if physical != base:
        raise ContractError("test.001d.invalid: fixture base is not canonical")
    root = base / str(fixture["name"])
    if root.exists() or root.is_symlink():
        raise ContractError("test.001d.io: fixture root already exists")
    try:
        root.mkdir(mode=0o700)
        for item in fixture["files"]:
            destination = root / str(item["path"])
            destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            destination.write_text(str(item["content"]), encoding="utf-8",
                                   newline="")
    except OSError as exc:
        if root.exists() and not root.is_symlink():
            shutil.rmtree(root)
        raise ContractError("test.001d.io: fixture materialization failed") from exc
    return FixtureWorkspace(base, root, _WORKSPACE_TOKEN)


def cleanup(workspace: FixtureWorkspace) -> None:
    if (not isinstance(workspace, FixtureWorkspace) or
            workspace._token is not _WORKSPACE_TOKEN):
        raise ContractError("test.001d.invalid: fixture cleanup owner is invalid")
    if workspace.cleaned:
        return
    if workspace.root.parent != workspace.base or workspace.root.is_symlink():
        raise ContractError("test.001d.invalid: fixture cleanup path escaped")
    try:
        shutil.rmtree(workspace.root)
    except OSError as exc:
        raise ContractError("test.001d.io: fixture cleanup failed") from exc
    workspace.cleaned = True


def fuzz(seconds: float, seed: int) -> int:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = 0
    rejected = 0
    base = {"schema": SCHEMA, "name": "case", "seed": seed,
            "target": "linux-x86_64", "files": [], "environment": []}
    raw = json.dumps(base, sort_keys=True, separators=(",", ":")).encode()
    while time.monotonic() < deadline or cases == 0:
        mutated = bytearray(raw)
        mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            validate(json.loads(bytes(mutated).decode("utf-8"),
                                object_pairs_hook=no_duplicates))
        except (ContractError, UnicodeError, json.JSONDecodeError):
            rejected += 1
        cases += 1
    if rejected == 0:
        raise ContractError("test.001d.invalid: fuzz produced no rejection")
    print(f"seed={seed} cases={cases} rejected={rejected}", file=sys.stderr)
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
        raise ContractError("test.001d.limit: benchmark exceeded hard 5% gate")
    print(f"warmups=5 samples=30 median_ms={measured:.6f} status=pass")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate", type=Path)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--benchmark-limit-ms", type=float)
    args = parser.parse_args()
    if args.fuzz_seconds < 0:
        raise ContractError("test.001d.invalid: negative fuzz duration")
    payload = validate(load(args.validate)) if args.validate else None
    if args.fuzz_seconds:
        fuzz(args.fuzz_seconds, args.seed)
    if args.benchmark_limit_ms is not None:
        if payload is None or args.benchmark_limit_ms <= 0:
            raise ContractError("test.001d.invalid: benchmark input is invalid")
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
