#!/usr/bin/env python3
"""Retain bounded diagnostic tails and the fixed CORE-004E signal witness."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import tempfile

_spec = importlib.util.spec_from_file_location(
    "core_004e_crash_witness", Path(__file__).with_name("core_004e_crash_witness.py"))
crash = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(crash)

MAX_FILES = 128
MAX_FILE_BYTES = 1024 * 1024
MAX_TOTAL_BYTES = 32 * 1024 * 1024
MAX_ENTRIES = 16384
# The index itself consumes one file and a reserved byte budget.
MAX_INDEX_BYTES = 256 * 1024

GATE_CHILD = re.compile(r"seen_[a-z0-9_]+\Z")
GATE_FILE = re.compile(
    r"seen_[a-z0-9_]+\.(?:log|status|before\.json|snapshot\.log|observation\.log|observation\.status)\Z")
GATE_METADATA = {"active-child.json", "checks.json", "harness-exit.log",
                 "accelerator-c-evidence.json"}


open_directory = crash.open_directory

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


def crash_candidates(root):
    """Only direct CORE-004E workdirs, or those immediately inside run.*."""
    base = root / ".seen/agent-tools"
    found = []
    examined = 0

    def scan(parent):
        nonlocal examined
        try:
            for entry in entries(root, parent):
                examined += 1
                if examined > MAX_ENTRIES:
                    raise ValueError("crash discovery exceeds its bound")
                yield parent / entry.name
        except FileNotFoundError:
            return

    def fixture(work):
        for path in scan(work):
            if path.name in {
                    f"crash-witness-{mode}-{label}"
                    for mode in ("fast", "thin-lto") for label in ("short", "long")}:
                found.append(path / "witness.json")
                if len(found) > MAX_FILES:
                    raise ValueError("too many crash witnesses")

    for path in scan(base):
        if path.name.startswith("core-004e."):
            fixture(path)
        elif path.name in {"safe-rebuild", "seen-core-004e",
                           "stage1-acceptance-quick", "stage1-acceptance-verify"}:
            for run in scan(path):
                if run.name.startswith("run."):
                    for work in scan(run):
                        if work.name.startswith("core-004e."):
                            fixture(work)
    return sorted(found)

def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate crash metadata key")
        result[key] = value
    return result


def publish_complete(destination, name, content):
    # Scratch lives outside the upload directory: interrupted writes are never
    # visible under a final artifact name, even when CI uploads after failure.
    descriptor, temporary = tempfile.mkstemp(prefix=".verification-publish-",
                                             dir=destination.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(content)
        os.replace(temporary, destination / name)
    finally:
        Path(temporary).unlink(missing_ok=True)



def collect_crashes(root, destination, records, budget):
    total = 0
    outcomes = []
    for metadata in crash_candidates(root):
        outcome = {"source": metadata.relative_to(root).as_posix()}
        outcomes.append(outcome)
        if len(records) >= MAX_FILES - 1 or total >= min(budget, crash.MAX_WITNESS_BYTES):
            outcome["omission"] = "budget"
            continue
        state, content = crash.snapshot(root, metadata, crash.MAX_METADATA_BYTES, keep=True)
        if "omission" in state:
            outcome["omission"] = state["omission"]
            continue
        try:
            report = json.loads(content, object_pairs_hook=unique_object)
            has_elf = crash.validate(report, metadata.parent.parent.relative_to(root).as_posix())
            if metadata.parent.name != f"crash-witness-{report['mode']}-{report['label']}":
                raise ValueError("witness directory does not match invocation")
        except (ValueError, UnicodeError, RecursionError, KeyError, TypeError):
            outcome["omission"] = "invalid-metadata"
            continue

        def save(path, data, suffix):
            nonlocal total
            if (len(records) >= MAX_FILES - 1 or
                    len(data) > min(budget, crash.MAX_WITNESS_BYTES) - total):
                return False
            name = f"crash-{len(records):03d}.{suffix}"
            try:
                publish_complete(destination, name, data)
            except OSError:
                outcome["omission"] = "publication-failed"
                return False
            records.append({"source": path.relative_to(root).as_posix(), "file": name,
                            "original_bytes": len(data), "retained_bytes": len(data),
                            "sha256": hashlib.sha256(data).hexdigest(),
                            "truncated": False, "changed_during_read": False,
                            "kind": "core-004e-crash-witness"})
            total += len(data)
            return True

        if not save(metadata, content, "json"):
            outcome.setdefault("omission", "budget")
            continue
        if not has_elf:
            outcome["omission"] = report["executable"]["omission"]
            continue
        expected = report["executable"]
        if (len(records) >= MAX_FILES - 1 or
                expected["bytes"] > min(budget, crash.MAX_WITNESS_BYTES) - total):
            outcome["omission"] = "budget"
            continue
        binary = metadata.parent / "program.elf"
        elf_state, elf = crash.snapshot(root, binary, crash.MAX_ELF_BYTES, keep=True, elf=True)
        if "omission" in elf_state:
            outcome["omission"] = elf_state["omission"]
        elif elf_state["sha256"] != expected["sha256"] or len(elf) != expected["bytes"]:
            outcome["omission"] = "identity-mismatch"
        else:
            # Recheck the marker after reading the ELF; never claim a group transaction.
            current, _ = crash.snapshot(root, metadata, crash.MAX_METADATA_BYTES)
            if current != state:
                outcome["omission"] = "changed"
            elif save(binary, elf, "elf"):
                outcome["retained_elf"] = True
            else:
                outcome.setdefault("omission", "budget")
    return total, outcomes


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
    budget = MAX_TOTAL_BYTES - MAX_INDEX_BYTES
    if budget < 0 or MAX_FILES < 1:
        raise ValueError("verification index reservation exceeds budget")
    total, crash_outcomes = collect_crashes(root, destination, records, budget)
    crash_files = len(records)
    for path in paths:
        if len(records) >= MAX_FILES - 1 or total >= budget:
            break
        try:
            original, content, snapshot = read_tail(root, path, min(MAX_FILE_BYTES, budget - total))
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
                "omitted_files": len(paths) - (len(records) - crash_files), "retained_bytes": total,
                "max_files": MAX_FILES, "max_file_bytes": MAX_FILE_BYTES,
                "max_total_bytes": MAX_TOTAL_BYTES, "unavailable_sources": unavailable,
                "crash_witnesses": crash_outcomes,
                "max_index_bytes": MAX_INDEX_BYTES,
                "snapshot_contract": "Independent observations, not an atomic execution snapshot or a success claim."}
    index = (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode()
    if len(index) > MAX_INDEX_BYTES:
        raise ValueError("verification index exceeds its reserved bound")
    (destination / "index.json").write_bytes(index)
    return manifest


if __name__ == "__main__":
    try:
        report = collect(Path(__file__).resolve().parents[1])
        print(f"Retained {len(report['files'])} bounded verification logs ({report['retained_bytes']} bytes)")
    except (OSError, ValueError) as error:
        raise SystemExit(f"verification-logs: {error}")
