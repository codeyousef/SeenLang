#!/usr/bin/env python3
"""Validate and render a bounded deterministic global-initialization plan."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import random
import sys
import time
from pathlib import Path

MAX_INPUT_BYTES = 1_048_576
MAX_MODULES = 4096
MAX_EDGES = 65_536
MAX_DEPTH = 4096
TOP_FIELDS = {"edges", "modules", "platform", "schema"}


def load_graph_contract() -> object:
    path = Path(__file__).with_name("check_import_graph.py")
    spec = importlib.util.spec_from_file_location("core003b_graph_contract", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load import-graph validation")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


GRAPH = load_graph_contract()
ContractError = GRAPH.ContractError


class ValidInitializationInput:
    def __init__(self, modules: tuple[str, ...], adjacency: tuple[tuple[int, ...], ...]) -> None:
        self.modules = modules
        self.adjacency = adjacency


def fail(code: str, message: str) -> None:
    raise ContractError(code, message)


def validate(document: object, max_modules: int, max_edges: int) -> ValidInitializationInput:
    if not 1 <= max_modules <= MAX_MODULES or not 0 <= max_edges <= MAX_EDGES:
        fail("limit", "global initialization limits are invalid")
    value = GRAPH.exact_object(document, TOP_FIELDS, "global initialization input")
    if value["schema"] != "seen-global-initialization-input-v1":
        fail("invalid", "global initialization schema is unsupported")
    if value["platform"] not in GRAPH.KNOWN_PLATFORMS:
        fail("platform", "global initialization platform is unsupported")
    raw_modules = value["modules"]
    if not isinstance(raw_modules, list) or not 1 <= len(raw_modules) <= max_modules:
        fail("limit", "global initialization module limit exceeded")
    modules = tuple(GRAPH.canonical_path(module) for module in raw_modules)
    if len(set(modules)) != len(modules):
        fail("invalid", "module identity is duplicated")
    raw_adjacency = value["edges"]
    if not isinstance(raw_adjacency, list) or len(raw_adjacency) != len(modules):
        fail("invalid", "global initialization edge shape is invalid")
    edge_count = 0
    adjacency: list[tuple[int, ...]] = []
    for raw_targets in raw_adjacency:
        if not isinstance(raw_targets, list):
            fail("invalid", "global initialization edge list is invalid")
        targets: list[int] = []
        for target in raw_targets:
            if isinstance(target, bool) or not isinstance(target, int):
                fail("invalid", "global initialization edge is not an index")
            if target < 0 or target >= len(modules) or target in targets:
                fail("invalid", "global initialization edge is invalid")
            targets.append(target)
            edge_count += 1
            if edge_count > max_edges:
                fail("limit", "global initialization edge limit exceeded")
        targets.sort(key=lambda index: modules[index].encode("utf-8"))
        adjacency.append(tuple(targets))
    return ValidInitializationInput(modules, tuple(adjacency))


def plan(value: ValidInitializationInput, max_depth: int, cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_depth <= MAX_DEPTH:
        fail("limit", "global initialization depth limit is invalid")
    if cancelled:
        fail("cancelled", "global initialization cancelled")
    colors = [0] * len(value.modules)
    order: list[int] = []
    roots = [0] + sorted(
        range(1, len(value.modules)),
        key=lambda index: value.modules[index].encode("utf-8"),
    )
    for root in roots:
        if colors[root] != 0:
            continue
        stack: list[tuple[int, int]] = [(root, 0)]
        colors[root] = 1
        while stack:
            if cancelled:
                fail("cancelled", "global initialization cancelled")
            node, offset = stack[-1]
            if offset >= len(value.adjacency[node]):
                colors[node] = 2
                order.append(node)
                stack.pop()
                continue
            target = value.adjacency[node][offset]
            stack[-1] = (node, offset + 1)
            if colors[target] == 1:
                fail("invalid", "global initialization dependency cycle detected")
            if colors[target] == 0:
                if len(stack) >= max_depth:
                    fail("limit", "global initialization depth limit exceeded")
                colors[target] = 1
                stack.append((target, 0))
    return {
        "module_count": len(value.modules),
        "order": order,
        "schema": "seen-global-initialization-plan-v1",
    }


def parse_and_plan(
    raw: bytes,
    max_bytes: int,
    max_modules: int,
    max_edges: int,
    max_depth: int,
    cancelled: bool = False,
) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_INPUT_BYTES or len(raw) > max_bytes:
        fail("limit", "global initialization input byte limit exceeded")
    if raw.startswith(b"\xef\xbb\xbf"):
        fail("invalid", "global initialization input must not contain a UTF-8 BOM")
    try:
        document = json.loads(raw.decode("utf-8"), object_pairs_hook=GRAPH.object_without_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"global initialization JSON is invalid: {error}")
    return plan(validate(document, max_modules, max_edges), max_depth, cancelled)


def fuzz(corpus: bytes, seconds: float, seed: int) -> None:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        mutated = bytearray(corpus)
        for _ in range(1 + rng.randrange(4)):
            action = rng.randrange(3)
            if action == 0 and mutated:
                del mutated[rng.randrange(len(mutated))]
            elif action == 1 and len(mutated) < MAX_INPUT_BYTES:
                mutated.insert(rng.randrange(len(mutated) + 1), rng.randrange(256))
            elif mutated:
                mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            result = parse_and_plan(
                bytes(mutated), MAX_INPUT_BYTES, MAX_MODULES, MAX_EDGES, MAX_DEPTH
            )
            json.loads((json.dumps(result, sort_keys=True) + "\n").encode())
        except ContractError:
            pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_INPUT_BYTES)
    parser.add_argument("--max-modules", type=int, default=MAX_MODULES)
    parser.add_argument("--max-edges", type=int, default=MAX_EDGES)
    parser.add_argument("--max-depth", type=int, default=MAX_DEPTH)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "fuzz-seconds must be between 0 and 300")
        raw = args.input.read_bytes()
        cancelled = args.test_cancel_after_read
        if cancelled and os.environ.get("SEEN_GLOBAL_INIT_TEST_HOOKS") != "1":
            fail("invalid", "cancellation hook is test-only")
        result = parse_and_plan(
            raw, args.max_bytes, args.max_modules, args.max_edges,
            args.max_depth, cancelled,
        )
        if args.fuzz_seconds:
            fuzz(raw, args.fuzz_seconds, args.seed)
            print(
                f"global-initialization: fuzz seed={args.seed} "
                f"seconds={args.fuzz_seconds:g} status=pass",
                file=sys.stderr,
            )
        json.dump(result, sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
    except ContractError as error:
        print(f"core.003b.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except (OSError, RuntimeError) as error:
        print(f"core.003b.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
