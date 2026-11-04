#!/usr/bin/env python3
"""Retain bounded diagnostic tails, never compiler IR or native artifacts."""
import hashlib
import json
import os
from pathlib import Path
import re
import stat

MAX_FILES = 128
MAX_FILE_BYTES = 1024 * 1024
MAX_TOTAL_BYTES = 32 * 1024 * 1024
MAX_ENTRIES = 16384

GATE_CHILD = re.compile(r"seen_[a-z0-9_]+\Z")
GATE_FILE = re.compile(
    r"seen_[a-z0-9_]+\.(?:log|status|before\.json|snapshot\.log|observation\.log|observation\.status)\Z")
GATE_METADATA = {"active-child.json", "checks.json", "harness-exit.log",
                 "accelerator-c-evidence.json"}


def open_directory(root, path):
    """Anchor discovery and reads at no-follow directory descriptors."""
    directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in path.relative_to(root).parts:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=directory)
            os.close(directory)
            directory = child
        return directory
    except BaseException:
        os.close(directory)
        raise


def entries(root, path):
    directory = open_directory(root, path)
    try:
        with os.scandir(directory) as source:
            yield from source
    finally:
        os.close(directory)


def active_child(content, original, snapshot):
    """A partial marker is retained as raw bytes, never trusted as a selector."""
    if original > 4096 or snapshot["truncated"] or snapshot["changed_during_read"]:
        return None
    try:
        value = json.loads(content)
    except (ValueError, UnicodeError, RecursionError):
        return None
    child = value.get("child") if type(value) is dict else None
    return child if type(child) is str and GATE_CHILD.fullmatch(child) else None


def candidates(root):
    base = root / ".seen/agent-tools/safe-rebuild"
    if any(parent.is_symlink() for parent in (root / ".seen", base.parent, base)):
        raise ValueError("unsafe verification source directory")
    if not base.exists():
        return []
    result = []
    examined = 0
    for entry in entries(root, base):
        run = base / entry.name
        examined += 1
        if examined > MAX_ENTRIES:
            raise ValueError("verification log discovery exceeds its bound")
        if not run.name.startswith("run."):
            continue
        if run.is_symlink() or not run.is_dir():
            raise ValueError("unsafe verification run directory")
        for entry in entries(root, run):
            child = run / entry.name
            examined += 1
            if examined > MAX_ENTRIES:
                raise ValueError("verification log discovery exceeds its bound")
            if (child.name == "safe-rebuild.trace.jsonl" or
                    (child.name.startswith("safe_rebuild") and child.suffix in (".log", ".metrics"))):
                result.append((2, child))
            elif child.name.startswith("p0-accel-c."):
                # Exactly one extra level, with an explicit diagnostic allowlist.
                # Do not walk children/, IR directories, caches, or binaries.
                active = None
                marker = child / "active-child.json"
                try:
                    original, content, snapshot = read_tail(root, marker, 4096)
                    active = active_child(content, original, snapshot)
                except FileNotFoundError:
                    pass
                for item in entries(root, child):
                    examined += 1
                    if examined > MAX_ENTRIES:
                        raise ValueError("verification log discovery exceeds its bound")
                    if item.name not in GATE_METADATA and not GATE_FILE.fullmatch(item.name):
                        continue
                    priority = 5
                    if item.name == "active-child.json":
                        priority = 0
                    elif active and item.name in (active + ".log", active + ".status"):
                        priority = 1
                    elif item.name in GATE_METADATA or (active and item.name.startswith(active + ".")):
                        priority = 3
                    result.append((priority, child / item.name))
            elif child.is_dir() and not child.name.startswith("seen_compile_"):
                active = child / "00-active.log"
                if active.exists() or active.is_symlink():
                    result.append((4, active))
    return [path for _, path in sorted(result)]


def read_tail(root, path, maximum):
    """Open every component without following symlinks, including parents."""
    directory = open_directory(root, path.parent)
    try:
        source = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
        try:
            info = os.fstat(source)
            if not stat.S_ISREG(info.st_mode):
                raise ValueError("verification log is not a regular file")
            offset = max(0, info.st_size - maximum)
            os.lseek(source, offset, os.SEEK_SET)
            content = os.read(source, maximum)
            after = os.fstat(source)
            try:
                current = os.stat(path.name, dir_fd=directory, follow_symlinks=False)
            except FileNotFoundError:
                current = None
            def identity(value):
                return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
            changed = (identity(info) != identity(after) or current is None or
                       identity(info) != identity(current) or len(content) != min(info.st_size, maximum))
            return info.st_size, content, {"truncated": offset > 0, "changed_during_read": changed}
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
    unavailable = []
    total = 0
    for path in paths:
        if len(records) == MAX_FILES or total == MAX_TOTAL_BYTES:
            break
        try:
            original, content, snapshot = read_tail(root, path, min(MAX_FILE_BYTES, MAX_TOTAL_BYTES - total))
        except FileNotFoundError:
            # Interrupted producers may remove/replace files between discovery
            # and opening. Keep the other diagnostics, without inventing bytes.
            if len(unavailable) < MAX_FILES:
                unavailable.append(path.relative_to(root).as_posix())
            continue
        name = f"log-{len(records):03d}.txt"
        (destination / name).write_bytes(content)
        records.append({"source": path.relative_to(root).as_posix(), "file": name,
                        "original_bytes": original, "retained_bytes": len(content),
                        "sha256": hashlib.sha256(content).hexdigest(), **snapshot})
        if path.name == "active-child.json":
            records[-1]["active_child"] = active_child(content, original, snapshot)
        if path.suffix == ".json":
            try:
                json.loads(content)
                records[-1]["incomplete_json"] = snapshot["truncated"] or snapshot["changed_during_read"]
            except (ValueError, UnicodeError, RecursionError):
                records[-1]["incomplete_json"] = True
        total += len(content)
    manifest = {"schema": "seen-verification-log-tails-v1", "files": records,
                "omitted_files": len(paths) - len(records), "retained_bytes": total,
                "max_files": MAX_FILES, "max_file_bytes": MAX_FILE_BYTES,
                "max_total_bytes": MAX_TOTAL_BYTES, "unavailable_sources": unavailable,
                "snapshot_contract": "Independent file tails, not an atomic gate snapshot or a success claim."}
    (destination / "index.json").write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n")
    return manifest


if __name__ == "__main__":
    try:
        report = collect(Path(__file__).resolve().parents[1])
        print(f"Retained {len(report['files'])} bounded verification logs ({report['retained_bytes']} bytes)")
    except (OSError, ValueError) as error:
        raise SystemExit(f"verification-logs: {error}")
