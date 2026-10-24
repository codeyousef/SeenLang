#!/usr/bin/env python3
"""Retain bounded build-log tails separately from large compiler IR artifacts."""
import hashlib
import json
import os
from pathlib import Path
import stat

MAX_FILES = 128
MAX_FILE_BYTES = 1024 * 1024
MAX_TOTAL_BYTES = 32 * 1024 * 1024
MAX_ENTRIES = 16384


def candidates(root):
    base = root / ".seen/agent-tools/safe-rebuild"
    if any(parent.is_symlink() for parent in (root / ".seen", base.parent, base)):
        raise ValueError("unsafe verification source directory")
    if not base.exists():
        return []
    result = []
    examined = 0
    for run in base.iterdir():
        examined += 1
        if examined > MAX_ENTRIES:
            raise ValueError("verification log discovery exceeds its bound")
        if not run.name.startswith("run."):
            continue
        if run.is_symlink() or not run.is_dir():
            raise ValueError("unsafe verification run directory")
        for child in run.iterdir():
            examined += 1
            if examined > MAX_ENTRIES:
                raise ValueError("verification log discovery exceeds its bound")
            if child.name.startswith("safe_rebuild") and child.suffix in (".log", ".metrics"):
                result.append((0, child))
            elif child.is_dir() and not child.name.startswith("seen_compile_"):
                active = child / "00-active.log"
                if active.exists() or active.is_symlink():
                    result.append((1, active))
    return [path for _, path in sorted(result)]


def read_tail(root, path, maximum):
    """Open every component without following symlinks, including parents."""
    parts = path.relative_to(root).parts
    directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in parts[:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=directory)
            os.close(directory)
            directory = child
        source = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
        try:
            info = os.fstat(source)
            if not stat.S_ISREG(info.st_mode):
                raise ValueError("verification log is not a regular file")
            os.lseek(source, max(0, info.st_size - maximum), os.SEEK_SET)
            return info.st_size, os.read(source, maximum)
        finally:
            os.close(source)
    finally:
        os.close(directory)


def collect(root):
    root = root.resolve()
    destination = root / "target/seen-build/verification-logs"
    for parent in (root / "target", destination.parent):
        if parent.is_symlink():
            raise ValueError("unsafe verification log destination")
        parent.mkdir(exist_ok=True)
    destination.mkdir()
    paths = candidates(root)
    records = []
    total = 0
    for path in paths:
        if len(records) == MAX_FILES or total == MAX_TOTAL_BYTES:
            break
        original, content = read_tail(root, path, min(MAX_FILE_BYTES, MAX_TOTAL_BYTES - total))
        name = f"log-{len(records):03d}.txt"
        (destination / name).write_bytes(content)
        records.append({"source": path.relative_to(root).as_posix(), "file": name,
                        "original_bytes": original, "retained_bytes": len(content),
                        "sha256": hashlib.sha256(content).hexdigest()})
        total += len(content)
    manifest = {"schema": "seen-verification-log-tails-v1", "files": records,
                "omitted_files": len(paths) - len(records), "retained_bytes": total,
                "max_files": MAX_FILES, "max_file_bytes": MAX_FILE_BYTES,
                "max_total_bytes": MAX_TOTAL_BYTES}
    (destination / "index.json").write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n")
    return manifest


if __name__ == "__main__":
    try:
        report = collect(Path(__file__).resolve().parents[1])
        print(f"Retained {len(report['files'])} bounded verification logs ({report['retained_bytes']} bytes)")
    except (OSError, ValueError) as error:
        raise SystemExit(f"verification-logs: {error}")
