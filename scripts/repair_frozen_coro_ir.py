#!/usr/bin/env python3
"""Adapt an unused frozen-bootstrap coroutine declaration to the host LLVM ABI."""

import re
import subprocess
import sys
from pathlib import Path


OLD_DECLARATION = "declare i1 @llvm.coro.end(ptr, i1, token)"
NEW_DECLARATION = "declare void @llvm.coro.end(ptr, i1, token)"
ACTIVE_CALL = re.compile(
    r"^\s*(?:%[-\w.]+\s*=\s*)?(?:call|invoke)\s+i1\s+@llvm\.coro\.end\s*\(",
    re.MULTILINE,
)


def repair(content: str, major: int) -> str:
    if major < 22 or OLD_DECLARATION not in content:
        return content
    declaration = re.compile(r"^" + re.escape(OLD_DECLARATION) + r"$", re.MULTILINE)
    if declaration.search(content) is None:
        return content
    if ACTIVE_CALL.search(content):
        raise ValueError("frozen IR has a live old-ABI llvm.coro.end call")
    return declaration.sub(NEW_DECLARATION, content)


def llvm_major(opt: str) -> int:
    result = subprocess.run(
        [opt, "--version"], capture_output=True, text=True, check=True, timeout=10
    )
    match = re.search(r"LLVM version\s+(\d+)", result.stdout)
    if match is None:
        raise ValueError("could not determine LLVM optimizer version")
    return int(match.group(1))


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: repair_frozen_coro_ir.py OPT FILE.ll", file=sys.stderr)
        return 2
    path = Path(sys.argv[2])
    try:
        original = path.read_text(encoding="utf-8")
        fixed = repair(original, llvm_major(sys.argv[1]))
        if fixed != original:
            path.write_text(fixed, encoding="utf-8")
            print(f"frozen coroutine declaration updated for host LLVM: {path}", file=sys.stderr)
    except (OSError, subprocess.SubprocessError, ValueError) as exc:
        print(f"frozen coroutine ABI repair failed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
