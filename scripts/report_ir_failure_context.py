#!/usr/bin/env python3
"""Print bounded context for compiler-owned IR named by an LLVM error log."""
from __future__ import annotations

import re
import sys
from pathlib import Path

MAX_IR_BYTES = 16 * 1024 * 1024
LOCATION = re.compile(r"(/[^\s:]+/seen_compile_[A-Za-z0-9_]+/seen_module_[0-9]+\.ll):(\d+):\d+: error:")


def failure_context(log: Path, project_root: Path) -> str:
    root = (project_root / ".seen").resolve(strict=True)
    if log.is_symlink() or not log.is_file():
        return ""
    with log.open("rb") as stream:
        stream.seek(max(0, log.stat().st_size - 16384))
        tail = stream.read(16384).decode("utf-8", errors="replace")
    sections = []
    seen = set()
    for match in list(LOCATION.finditer(tail))[-4:]:
        candidate = Path(match[1])
        try:
            relative = candidate.relative_to(root)
            current = root
            for component in relative.parts:
                current /= component
                if current.is_symlink():
                    raise ValueError("symlink in compiler IR path")
            candidate.resolve(strict=True).relative_to(root)
            if not candidate.is_file() or candidate.stat().st_size > MAX_IR_BYTES:
                continue
            line_number = int(match[2])
            identity = (str(candidate), line_number)
            if identity in seen:
                continue
            seen.add(identity)
            with candidate.open("rb") as stream:
                data = stream.read(MAX_IR_BYTES + 1)
            if len(data) > MAX_IR_BYTES:
                continue
            lines = data.decode("utf-8", errors="replace").splitlines()
            if line_number < 1 or line_number > len(lines):
                continue
            header = ""
            for line in reversed(lines[:line_number]):
                if line.startswith("define "):
                    header = line[:512]
                    break
            first = max(0, line_number - 25)
            last = min(len(lines), line_number + 8)
            snippet = "\n".join(f"{i + 1}: {lines[i][:512]}" for i in range(first, last))
            sections.append(f"IR failure context: {candidate.name}:{line_number}\n{header}\n{snippet}")
        except (OSError, ValueError):
            continue
    return "\n".join(sections)


def main() -> None:
    try:
        result = failure_context(Path(sys.argv[1]), Path(sys.argv[2]))
        if result:
            print(result)
    except (OSError, ValueError, IndexError):
        pass  # Diagnostics never replace the original gate failure.


if __name__ == "__main__":
    main()
