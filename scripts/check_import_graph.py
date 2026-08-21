#!/usr/bin/env python3
"""Validate and canonically resolve a bounded seen-import-graph-v1 document."""

from __future__ import annotations

import argparse
import json
import os
import random
import sys
import time
from pathlib import Path

MAX_GRAPH_BYTES = 1_048_576
MAX_MODULES = 4096
MAX_EDGES = 65_536
MAX_DEPTH = 4096
MAX_PATH_BYTES = 4096
TOP_FIELDS = {"edges", "modules", "platform", "root", "schema"}
EDGE_FIELDS = {"from", "to"}
KNOWN_PLATFORMS = {
    "android-arm64",
    "ios-arm64",
    "ios-sim-arm64",
    "linux-arm64",
    "linux-riscv64",
    "linux-x86_64",
    "macos",
    "macos-arm64",
    "macos-x86_64",
    "windows",
    "windows-x86_64",
}


class ContractError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise ContractError(code, message)


def object_without_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail("invalid", f"duplicate field {key!r}")
        result[key] = value
    return result


def exact_object(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        fail("invalid", f"{label} has missing or unknown fields")
    return value


def canonical_path(value: object) -> str:
    if not isinstance(value, str):
        fail("invalid", "module identity must be a string")
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError as error:
        fail("invalid", f"module identity is not UTF-8: {error}")
    if not encoded or len(encoded) > MAX_PATH_BYTES or b"\x00" in encoded:
        fail("limit", "module identity byte limit exceeded")
    normalized = value.replace("\\", "/")
    if value.startswith(("/", "\\")) or "\\" in value:
        fail("invalid", "module identity is not canonical")
    segments = normalized.split("/")
    if any(segment in {"", ".", ".."} for segment in segments):
        fail("invalid", "module identity contains traversal or an empty segment")
    return value


class ValidGraph:
    def __init__(
        self,
        modules: tuple[str, ...],
        adjacency: tuple[tuple[int, ...], ...],
        edge_count: int,
    ) -> None:
        self.modules = modules
        self.adjacency = adjacency
        self.edge_count = edge_count


def validate(document: object, max_modules: int, max_edges: int) -> ValidGraph:
    if not 1 <= max_modules <= MAX_MODULES or not 0 <= max_edges <= MAX_EDGES:
        fail("limit", "graph limits are invalid")
    graph = exact_object(document, TOP_FIELDS, "graph")
    if graph["schema"] != "seen-import-graph-v1":
        fail("invalid", "graph schema is unsupported")
    if graph["platform"] not in KNOWN_PLATFORMS:
        fail("platform", "graph platform is unsupported")
    raw_modules = graph["modules"]
    if not isinstance(raw_modules, list) or not 1 <= len(raw_modules) <= max_modules:
        fail("limit", "graph module limit exceeded")
    modules = tuple(canonical_path(module) for module in raw_modules)
    if len(set(modules)) != len(modules):
        fail("invalid", "module identity is duplicated")
    root = canonical_path(graph["root"])
    if root != modules[0]:
        fail("invalid", "root must be the first module identity")
    module_indexes = {module: index for index, module in enumerate(modules)}
    raw_edges = graph["edges"]
    if not isinstance(raw_edges, list) or len(raw_edges) > max_edges:
        fail("limit", "graph edge limit exceeded")
    adjacency_lists: list[list[int]] = [[] for _ in modules]
    seen_edges: set[tuple[int, int]] = set()
    for raw_edge in raw_edges:
        edge = exact_object(raw_edge, EDGE_FIELDS, "edge")
        source = canonical_path(edge["from"])
        target = canonical_path(edge["to"])
        if source not in module_indexes or target not in module_indexes:
            fail("invalid", "graph edge references an unknown module")
        pair = (module_indexes[source], module_indexes[target])
        if pair in seen_edges:
            fail("invalid", "graph edge is duplicated")
        seen_edges.add(pair)
        adjacency_lists[pair[0]].append(pair[1])
    for targets in adjacency_lists:
        targets.sort(key=lambda index: modules[index].encode("utf-8"))
    return ValidGraph(
        modules=modules,
        adjacency=tuple(tuple(targets) for targets in adjacency_lists),
        edge_count=len(raw_edges),
    )


def resolve(graph: ValidGraph, max_depth: int, cancelled: bool = False) -> dict[str, object]:
    if not 1 <= max_depth <= MAX_DEPTH:
        fail("limit", "graph depth limit is invalid")
    if cancelled:
        fail("cancelled", "import graph resolution cancelled")
    colors = [0] * len(graph.modules)
    order: list[int] = []
    roots = [0] + sorted(
        range(1, len(graph.modules)), key=lambda index: graph.modules[index].encode("utf-8")
    )
    for root in roots:
        if colors[root] != 0:
            continue
        stack: list[tuple[int, int]] = [(root, 0)]
        colors[root] = 1
        order.append(root)
        while stack:
            if cancelled:
                fail("cancelled", "import graph resolution cancelled")
            node, offset = stack[-1]
            if offset >= len(graph.adjacency[node]):
                colors[node] = 2
                stack.pop()
                continue
            target = graph.adjacency[node][offset]
            stack[-1] = (node, offset + 1)
            if colors[target] == 1:
                cycle_start = next(i for i, frame in enumerate(stack) if frame[0] == target)
                cycle = [graph.modules[frame[0]] for frame in stack[cycle_start:]]
                cycle.append(graph.modules[target])
                fail("cycle", "cyclic import detected: " + " -> ".join(cycle))
            if colors[target] == 0:
                if len(stack) >= max_depth:
                    fail("limit", "graph depth limit exceeded")
                colors[target] = 1
                order.append(target)
                stack.append((target, 0))
    return {
        "edge_count": graph.edge_count,
        "module_count": len(graph.modules),
        "order": order,
        "schema": "seen-import-graph-v1",
    }


def parse_and_resolve(
    raw: bytes,
    max_bytes: int,
    max_modules: int,
    max_edges: int,
    max_depth: int,
    cancelled: bool = False,
) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_GRAPH_BYTES or len(raw) > max_bytes:
        fail("limit", "graph byte limit exceeded")
    if raw.startswith(b"\xef\xbb\xbf"):
        fail("invalid", "graph must not contain a UTF-8 BOM")
    try:
        document = json.loads(raw.decode("utf-8"), object_pairs_hook=object_without_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"graph JSON is invalid: {error}")
    return resolve(validate(document, max_modules, max_edges), max_depth, cancelled)


def fuzz(corpus: bytes, seconds: float, seed: int) -> None:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        mutated = bytearray(corpus)
        for _ in range(1 + rng.randrange(4)):
            action = rng.randrange(3)
            if action == 0 and mutated:
                del mutated[rng.randrange(len(mutated))]
            elif action == 1 and len(mutated) < MAX_GRAPH_BYTES:
                mutated.insert(rng.randrange(len(mutated) + 1), rng.randrange(256))
            elif mutated:
                mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            result = parse_and_resolve(
                bytes(mutated), MAX_GRAPH_BYTES, MAX_MODULES, MAX_EDGES, MAX_DEPTH
            )
            canonical = (json.dumps(result, indent=2, sort_keys=True) + "\n").encode()
            json.loads(canonical)
        except ContractError:
            pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("graph", type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_GRAPH_BYTES)
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
        raw = args.graph.read_bytes()
        cancelled = args.test_cancel_after_read
        if cancelled and os.environ.get("SEEN_IMPORT_GRAPH_TEST_HOOKS") != "1":
            fail("invalid", "cancellation hook is test-only")
        result = parse_and_resolve(
            raw,
            args.max_bytes,
            args.max_modules,
            args.max_edges,
            args.max_depth,
            cancelled,
        )
        if args.fuzz_seconds:
            fuzz(raw, args.fuzz_seconds, args.seed)
            print(
                f"import-graph: fuzz seed={args.seed} "
                f"seconds={args.fuzz_seconds:g} status=pass",
                file=sys.stderr,
            )
        json.dump(result, sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
    except ContractError as error:
        print(f"core.003a.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.003a.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
