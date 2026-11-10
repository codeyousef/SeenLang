#!/usr/bin/env python3
"""CORE-004E signal evidence only; hashes bind observations, not execution atomicity.

No source contents, ambient environment, IR, core dumps, or executable reruns.
The harness calls prepare immediately before its existing subprocess and retain
only after that subprocess returns. Publication is metadata-last, not durable or
an atomic multi-file snapshot. A changed executable is omitted, never tailed.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat

MAX_ELF_BYTES = 24 * 1024 * 1024
MAX_METADATA_BYTES = 64 * 1024
MAX_IDENTITY_BYTES = 512 * 1024 * 1024
MAX_WITNESS_BYTES = 24 * 1024 * 1024 + 4 * MAX_METADATA_BYTES
SCHEMA = "seen-core-004e-crash-witness-v1"
ENVIRONMENT = {
    "SEEN_DETERMINISTIC": "1", "SOURCE_DATE_EPOCH": "1700000000",
    "SEEN_DETERMINISTIC_SEED": "1101", "SEEN_HASH_SEED": "1101",
    "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "TZ": "UTC",
}
SOURCES = {
    "fixture": "tests/fixtures/core-004e/arguments_contract.seen",
    "policy": "seen_std/src/determinism/policy.seen",
    "context": "seen_std/src/determinism/context.seen",
    "runtime": "seen_runtime/seen_runtime.c",
}
STAT_KEYS = ("device", "inode", "size", "mtime_ns", "ctime_ns", "mode")
OMISSIONS = {"unsafe-or-unavailable", "over-limit", "changed", "not-elf"}


def open_directory(root, path):
    """Anchor every component at a no-follow descriptor; reject lexical escapes."""
    relative = path.relative_to(root)
    if ".." in relative.parts:
        raise ValueError("path escapes root")
    directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in relative.parts:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=directory)
            os.close(directory)
            directory = child
        return directory
    except BaseException:
        os.close(directory)
        raise


def identity(info):
    return dict(zip(STAT_KEYS, (info.st_dev, info.st_ino, info.st_size,
                               info.st_mtime_ns, info.st_ctime_ns, info.st_mode)))


def snapshot(root, path, maximum, *, keep=False, elf=False):
    """Bounded complete read, with fd and pathname identities before and after."""
    content = []
    try:
        directory = open_directory(root, path.parent)
        try:
            source = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                             dir_fd=directory)
            try:
                before = os.fstat(source)
                if not stat.S_ISREG(before.st_mode):
                    return {"omission": "unsafe-or-unavailable"}, None
                if before.st_size > maximum:
                    return {"omission": "over-limit"}, None
                digest = hashlib.sha256()
                remaining = before.st_size
                prefix = b""
                while remaining:
                    block = os.read(source, min(65536, remaining))
                    if not block:
                        break
                    if not prefix:
                        prefix = block[:64]
                    digest.update(block)
                    if keep:
                        content.append(block)
                    remaining -= len(block)
                after = os.fstat(source)
                current = os.stat(path.name, dir_fd=directory, follow_symlinks=False)
                if (remaining or identity(before) != identity(after) or
                        identity(before) != identity(current)):
                    return {"omission": "changed"}, None
                if elf and not (len(prefix) >= 52 and prefix[:4] == b"\x7fELF" and
                                prefix[4] in (1, 2) and prefix[5] in (1, 2) and
                                (prefix[4] != 2 or len(prefix) >= 64)):
                    return {"omission": "not-elf"}, None
                return {"stat": identity(before), "sha256": digest.hexdigest()}, (
                    b"".join(content) if keep else None)
            finally:
                os.close(source)
        finally:
            os.close(directory)
    except (OSError, ValueError):
        return {"omission": "unsafe-or-unavailable"}, None


def invocation(work, mode, label):
    if mode not in ("fast", "thin-lto") or label not in ("short", "long"):
        raise ValueError("unknown argument fixture invocation")
    extra = [] if label == "short" else [
        f"unused-{index}-" + "x" * 96 for index in range(64)]
    return [str(work / f"arguments-{mode}"), "kept", *extra], dict(ENVIRONMENT)


def source_paths(work, mode, compiler):
    return {**SOURCES, "compiler": compiler,
            "program": f"{work}/arguments-{mode}"}


def observe(root, paths):
    return {role: snapshot(root, root / path,
                          MAX_ELF_BYTES if role == "program" else MAX_IDENTITY_BYTES,
                          elf=role == "program")[0]
            for role, path in paths.items()}

def anchored_compiler(root, compiler):
    if compiler.is_relative_to(root):
        return compiler.relative_to(root).as_posix()
    if compiler not in (Path("/tmp/seen_quick_rebuild"), Path("/tmp/seen_verify_rebuild")):
        raise ValueError("unsupported external compiler identity")
    artifact = Path(os.environ["SEEN_ARTIFACT_ROOT"])
    directory = open_directory(root, artifact)
    try:
        private = os.fstat(directory)
        temporary = os.stat("/tmp", follow_symlinks=False)
        if (private.st_dev, private.st_ino) != (temporary.st_dev, temporary.st_ino):
            raise ValueError("compiler alias is not the project-backed private /tmp")
        anchored = artifact / compiler.name
        state, _ = snapshot(root, anchored, MAX_IDENTITY_BYTES)
        if ("omission" in state or
                identity(os.stat(compiler, follow_symlinks=False)) != state["stat"]):
            raise ValueError("compiler alias identity differs")
        return anchored.relative_to(root).as_posix()
    finally:
        os.close(directory)



def prepare(root, work, mode, label, compiler):
    invocation(work, mode, label)
    work_relative = work.relative_to(root).as_posix()
    compiler_relative = anchored_compiler(root, compiler)
    paths = source_paths(work_relative, mode, compiler_relative)
    if any(".." in Path(path).parts for path in paths.values()):
        raise ValueError("identity escaped root")
    return {"paths": paths, "before": observe(root, paths)}


def create_file(directory, name, content):
    descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                         0o600, dir_fd=directory)
    with os.fdopen(descriptor, "wb") as output:
        output.write(content)


def retain(root, work, mode, label, prepared, returncode):
    """Never run a process, reinterpret an exit code, or replace gate failure."""
    if type(returncode) is not int or returncode >= 0:
        return None
    if -returncode not in signal.valid_signals():
        raise ValueError("invalid subprocess signal")
    argv, environment = invocation(work, mode, label)
    paths = prepared["paths"]
    after = observe(root, paths)
    program, content = snapshot(root, root / paths["program"], MAX_ELF_BYTES,
                                keep=True, elf=True)
    before = prepared["before"]
    retained = ("omission" not in program and before["program"] == program and
                after["program"] == program)
    if not retained:
        content = None
    report = {
        "schema": SCHEMA, "mode": mode, "label": label,
        "returncode": returncode, "signal": -returncode,
        "command": {"argv": [paths["program"], *argv[1:]],
                    "cwd": Path.cwd().relative_to(root).as_posix(),
                    "environment": environment},
        "compile": {"compiler": paths["compiler"], "source": SOURCES["fixture"],
                    "flags": (["--fast"] if mode == "fast" else
                              ["--release", "--lto=thin"]) + ["--no-cache", "--no-fork"]},
        "identities": {role: {"path": path, "before": before[role], "after": after[role]}
                       for role, path in paths.items()},
        "executable": ({"file": "program.elf", "bytes": len(content),
                        "sha256": program["sha256"]} if retained else
                       {"omission": program.get("omission", "changed")}),
    }
    metadata = (json.dumps(report, sort_keys=True, indent=2) + "\n").encode()
    if len(metadata) > MAX_METADATA_BYTES:
        raise ValueError("crash metadata exceeds bound")
    parent = open_directory(root, work)
    try:
        name = f"crash-witness-{mode}-{label}"
        os.mkdir(name, 0o700, dir_fd=parent)
        directory = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)
        try:
            if retained:
                create_file(directory, "program.elf", content)
            create_file(directory, "witness.json", metadata)
        finally:
            os.close(directory)
    finally:
        os.close(parent)
    return report


def valid_state(value):
    if type(value) is not dict:
        return False
    if set(value) == {"omission"}:
        return value["omission"] in OMISSIONS
    return (set(value) == {"stat", "sha256"} and
            type(value["sha256"]) is str and re.fullmatch(r"[0-9a-f]{64}", value["sha256"]) and
            type(value["stat"]) is dict and set(value["stat"]) == set(STAT_KEYS) and
            all(type(item) is int for item in value["stat"].values()) and
            0 <= value["stat"]["size"] <= MAX_IDENTITY_BYTES)


def validate(report, work):
    """Only the fixed fixture replay/identity vocabulary may enter the upload."""
    if type(report) is not dict or set(report) != {
            "schema", "mode", "label", "returncode", "signal", "command", "compile",
            "identities", "executable"} or report["schema"] != SCHEMA:
        raise ValueError("invalid crash witness schema")
    mode, label = report["mode"], report["label"]
    argv, environment = invocation(Path(work), mode, label)
    command = report["command"]
    if (type(command) is not dict or set(command) != {"argv", "cwd", "environment"} or
            command["argv"] != argv or command["environment"] != environment or
            type(command["cwd"]) is not str or not 0 < len(command["cwd"]) <= 512 or
            Path(command["cwd"]).is_absolute() or ".." in Path(command["cwd"]).parts or
            any(ord(character) < 32 for character in command["cwd"])):
        raise ValueError("non-whitelisted crash replay")
    if (type(report["returncode"]) is not int or type(report["signal"]) is not int or
            report["signal"] not in signal.valid_signals() or report["signal"] <= 0 or
            report["returncode"] != -report["signal"]):
        raise ValueError("not a native signal failure")
    compilation = report["compile"]
    if type(compilation) is not dict or set(compilation) != {"compiler", "source", "flags"}:
        raise ValueError("invalid compiler identity")
    compiler = compilation["compiler"]
    if (type(compiler) is not str or not 0 < len(compiler) <= 512 or
            Path(compiler).is_absolute() or ".." in Path(compiler).parts or
            any(ord(character) < 32 for character in compiler)):
        raise ValueError("unsafe compiler path")
    expected_compile = {"compiler": compiler, "source": SOURCES["fixture"],
                        "flags": (["--fast"] if mode == "fast" else
                                  ["--release", "--lto=thin"]) + ["--no-cache", "--no-fork"]}
    if compilation != expected_compile:
        raise ValueError("non-whitelisted compilation")
    paths = source_paths(work, mode, compiler)
    identities = report["identities"]
    if type(identities) is not dict or set(identities) != set(paths):
        raise ValueError("non-whitelisted identities")
    for role, path in paths.items():
        row = identities[role]
        if (type(row) is not dict or set(row) != {"path", "before", "after"} or
                row["path"] != path or not valid_state(row["before"]) or
                not valid_state(row["after"])):
            raise ValueError("invalid file identity")
    executable = report["executable"]
    if type(executable) is not dict:
        raise ValueError("invalid executable record")
    if set(executable) == {"omission"} and executable["omission"] in OMISSIONS:
        return False
    program = identities["program"]
    if (set(executable) != {"file", "bytes", "sha256"} or executable["file"] != "program.elf" or
            type(executable["bytes"]) is not int or not 0 < executable["bytes"] <= MAX_ELF_BYTES or
            "omission" in program["before"] or program["before"] != program["after"] or
            executable["bytes"] != program["before"]["stat"]["size"] or
            executable["sha256"] != program["before"]["sha256"]):
        raise ValueError("unbound executable record")
    return True
