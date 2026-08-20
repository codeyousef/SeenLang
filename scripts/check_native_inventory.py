#!/usr/bin/env python3
"""Generate or verify Seen's deterministic foreign-symbol/backend inventory."""

from __future__ import annotations

import argparse
import json
import os
import re
import signal
import sys
import tempfile
from pathlib import Path

SCAN_ROOTS = ("compiler_seen/src", "seen_std/src")
MAX_FILES_HARD = 8192
MAX_BYTES_HARD = 64 * 1024 * 1024
MAX_SYMBOLS_HARD = 8192
EXTERN = re.compile(
    r'^\s*extern(?:\s+"(?P<abi>[A-Za-z][A-Za-z0-9_.-]{0,31})")?'
    r"\s+fun\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
    r"(?:<[^>\n]{1,256}>)?\s*\("
)
BACKEND = re.compile(
    r"^\s*class\s+(?P<name>[A-Z][A-Za-z0-9]*)Backend(?:\s+extends\s+[A-Za-z0-9_]+)?\s*\{"
)
SHIPPED_BACKEND = re.compile(r"Supported backend:\s*([a-z][a-z0-9_-]*)")


class InventoryError(ValueError):
    pass


def fail(message: str) -> None:
    raise InventoryError(message)


def backend_name(class_prefix: str) -> str:
    return class_prefix.lower()


def safe_files(root: Path, maximum: int) -> list[Path]:
    files: list[Path] = []
    for relative_root in SCAN_ROOTS:
        directory = root / relative_root
        if not directory.is_dir() or directory.is_symlink():
            fail(f"missing or unsafe scan root: {relative_root}")
        for path in directory.rglob("*.seen"):
            if path.is_symlink() or not path.is_file():
                fail(f"unsafe source entry: {path.relative_to(root).as_posix()}")
            resolved = path.resolve(strict=True)
            try:
                resolved.relative_to(root)
            except ValueError:
                fail(f"source escaped repository root: {path}")
            files.append(path)
            if len(files) > maximum:
                fail(f"source file limit exceeded ({maximum})")
    files.sort(key=lambda path: path.relative_to(root).as_posix().encode("utf-8"))
    return files


def repository_path(root: Path, requested: Path, must_exist: bool) -> Path:
    candidate = requested if requested.is_absolute() else root / requested
    if must_exist:
        resolved = candidate.resolve(strict=True)
    else:
        if candidate.is_symlink():
            fail(f"output may not be a symbolic link: {candidate}")
        resolved_parent = candidate.parent.resolve(strict=True)
        resolved = resolved_parent / candidate.name
    try:
        resolved.relative_to(root)
    except ValueError:
        fail(f"path escaped repository root: {requested}")
    return resolved


def build_inventory(
    root: Path, max_files: int, max_bytes: int, max_symbols: int, cancel_after: int
) -> dict[str, object]:
    symbols: dict[tuple[str, str], set[str]] = {}
    backend_sources: dict[str, set[str]] = {}
    shipped: set[str] = set()
    total_bytes = 0
    files = safe_files(root, max_files)

    for index, path in enumerate(files, 1):
        if cancel_after and index > cancel_after:
            raise KeyboardInterrupt
        relative = path.relative_to(root).as_posix()
        data = path.read_bytes()
        total_bytes += len(data)
        if total_bytes > max_bytes:
            fail(f"source byte limit exceeded ({max_bytes})")
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError as error:
            fail(f"non-UTF-8 source: {relative}: {error}")
        for number, raw_line in enumerate(text.splitlines(), 1):
            line = raw_line.split("//", 1)[0]
            match = EXTERN.match(line)
            if match:
                abi = match.group("abi") or "Seen"
                key = (match.group("name"), abi)
                symbols.setdefault(key, set()).add(relative)
                if len(symbols) > max_symbols:
                    fail(f"foreign-symbol limit exceeded ({max_symbols})")
            elif re.match(r"^\s*extern(?:\s|$)", line) and "fun" in line:
                fail(f"malformed extern declaration: {relative}:{number}")

            backend = BACKEND.match(line)
            if backend:
                name = backend_name(backend.group("name"))
                if name != "backend":
                    backend_sources.setdefault(name, set()).add(relative)
            if relative == "compiler_seen/src/main_compiler.seen":
                shipped_match = SHIPPED_BACKEND.search(line)
                if shipped_match:
                    shipped.add(shipped_match.group(1))

    if not symbols:
        fail("no foreign symbols discovered")
    if not backend_sources:
        fail("no backend implementations discovered")
    if not shipped:
        fail("no shipped backend declaration discovered")
    unknown = shipped.difference(backend_sources)
    if unknown:
        fail(f"shipped backend lacks an implementation: {sorted(unknown)[0]}")

    foreign_symbols = [
        {"name": name, "abi": abi, "sources": sorted(sources)}
        for (name, abi), sources in sorted(symbols.items())
    ]
    backends = [
        {
            "name": name,
            "status": "shipped" if name in shipped else "source-only",
            "sources": sorted(sources),
        }
        for name, sources in sorted(backend_sources.items())
    ]
    return {
        "version": 1,
        "scan_roots": list(SCAN_ROOTS),
        "foreign_symbols": foreign_symbols,
        "backends": backends,
    }


def canonical_bytes(inventory: dict[str, object]) -> bytes:
    return (json.dumps(inventory, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def atomic_write(path: Path, content: bytes) -> None:
    parent = path.parent
    if not parent.is_dir() or parent.is_symlink():
        fail(f"unsafe output directory: {parent}")
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def positive_bounded(value: str, maximum: int, name: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError:
        raise argparse.ArgumentTypeError(f"{name} must be an integer")
    if parsed < 1 or parsed > maximum:
        raise argparse.ArgumentTypeError(f"{name} must be between 1 and {maximum}")
    return parsed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", type=Path)
    mode.add_argument("--write", type=Path)
    parser.add_argument("--max-files", default="4096")
    parser.add_argument("--max-bytes", default=str(32 * 1024 * 1024))
    parser.add_argument("--max-symbols", default="4096")
    parser.add_argument("--test-cancel-after-files", type=int, default=0, help=argparse.SUPPRESS)
    args = parser.parse_args()

    try:
        max_files = positive_bounded(args.max_files, MAX_FILES_HARD, "max-files")
        max_bytes = positive_bounded(args.max_bytes, MAX_BYTES_HARD, "max-bytes")
        max_symbols = positive_bounded(args.max_symbols, MAX_SYMBOLS_HARD, "max-symbols")
        if args.test_cancel_after_files:
            if os.environ.get("SEEN_NATIVE_INVENTORY_TEST_HOOKS") != "1":
                fail("cancellation test hook is disabled")
            if args.test_cancel_after_files < 1:
                fail("cancellation test hook must be positive")
        root = args.root.resolve(strict=True)
        if not root.is_dir():
            fail("repository root is not a directory")
        content = canonical_bytes(
            build_inventory(
                root, max_files, max_bytes, max_symbols, args.test_cancel_after_files
            )
        )
        if args.check:
            check_path = repository_path(root, args.check, must_exist=True)
            expected = check_path.read_bytes()
            if expected != content:
                fail(f"inventory is stale: {args.check}")
            print(f"native-inventory: valid {args.check}")
        elif args.write:
            output = repository_path(root, args.write, must_exist=False)
            atomic_write(output, content)
            print(f"native-inventory: wrote {output}")
        else:
            sys.stdout.buffer.write(content)
    except KeyboardInterrupt:
        print("native-inventory: cancelled", file=sys.stderr)
        return 130
    except (OSError, InventoryError, argparse.ArgumentTypeError) as error:
        print(f"native-inventory: {error}", file=sys.stderr)
        return 1
    return 0


def cancel(_signum: int, _frame: object) -> None:
    raise KeyboardInterrupt


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, cancel)
    sys.exit(main())
