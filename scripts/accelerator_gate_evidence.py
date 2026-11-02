#!/usr/bin/env python3
"""Bind actual accelerator-gate results to bounded source/log/scope evidence.

Reports have atomic visibility within one owned run directory, not a claim of
power-loss durability or production certification.
"""
import argparse
import fcntl
import hashlib
import io
from itertools import islice
import json
import os
import re
import resource
import subprocess
import stat
import sys
import tempfile
from pathlib import Path
import tarfile
import uuid

from gpu_certification_io import read_bounded

ROOT = Path(__file__).resolve().parent.parent
MAX_DOCUMENT = 256 * 1024
MAX_SOURCE = 64 * 1024**2
MAX_LOGS = 128 * 1024**2
NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,95}\Z")
CLASSIFICATIONS = {"unsupported", "compile-only", "experimental-hardware", "verified"}
EXECUTIONS = {"none", "native-cpu", "native-gpu", "official-validation"}
MAX_ARCHIVE = MAX_LOGS + 16 * 1024**2
RETAINED_PER_PHASE = 2
BUNDLE_NAME = re.compile(r"gate-([abcd])-[0-9a-f]{32}\.tar\.gz\Z")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path, ceiling):
    content = read_bounded(path, ceiling)
    return {"sha256": hashlib.sha256(content).hexdigest(), "bytes": len(content)}


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        require(key not in value, "duplicate evidence field")
        value[key] = item
    return value


def invalid_constant(value):
    raise ValueError("nonfinite evidence number")


def git(*arguments):
    return subprocess.run(["git", *arguments], cwd=ROOT, check=True,
                          capture_output=True, timeout=15).stdout


def source_identity():
    commit = git("rev-parse", "HEAD").decode().strip()
    changed = set(git("diff", "--name-only", "-z", "HEAD", "--").split(b"\0"))
    changed.update(git("ls-files", "--others", "--exclude-standard", "-z").split(b"\0"))
    changed.discard(b"")
    require(len(changed) <= 512, "source change catalog exceeds its ceiling")
    catalog = {}
    remaining = MAX_SOURCE
    for raw in sorted(changed):
        name = raw.decode("utf-8")
        relative = Path(name)
        require(not relative.is_absolute() and ".." not in relative.parts, "invalid changed source path")
        path = ROOT / relative
        if not path.exists() and not path.is_symlink():
            catalog[name] = {"deleted": True}
            continue
        require(path.resolve().is_relative_to(ROOT), "changed source escaped its repository")
        item = digest(path, min(32 * 1024**2, remaining))
        remaining -= item["bytes"]
        catalog[name] = item
    return {"commit": commit, "clean": not catalog, "changes": catalog}


def containment():
    subprocess.run(["bash", str(ROOT / "scripts/run_in_hard_memory_scope.sh"), "--verify-only"],
                   check=True, timeout=30)
    relative = next(line.split("::", 1)[1] for line in Path("/proc/self/cgroup").read_text().splitlines()
                    if line.startswith("0::"))
    directory = Path("/sys/fs/cgroup") / relative.lstrip("/")
    values = {name: int((directory / name).read_text().strip())
              for name in ("memory.max", "memory.swap.max", "memory.oom.group", "pids.max")}
    require(values["memory.max"] > 0 and values["pids.max"] > 0 and
            values["memory.swap.max"] == 0 and values["memory.oom.group"] == 1,
            "accelerator evidence requires finite aggregate containment with zero swap")
    return {"path": relative, **values}


def validate_checks(checks, artifacts):
    require(type(checks) is list and 1 <= len(checks) <= 128, "invalid accelerator check count")
    names = set()
    remaining = MAX_LOGS
    snapshots = {}
    physical = {}
    result = []
    for row in checks:
        require(type(row) is dict and set(row) ==
                {"name", "exit_status", "classification", "execution", "required", "logs", "details"},
                "invalid accelerator check fields")
        name = row["name"]
        require(type(name) is str and NAME.fullmatch(name) and name not in names, "invalid or duplicate accelerator check name")
        names.add(name)
        require(type(row["exit_status"]) is int and 0 <= row["exit_status"] <= 255, "invalid native exit status")
        require(type(row["classification"]) is str and type(row["execution"]) is str and
                row["classification"] in CLASSIFICATIONS and row["execution"] in EXECUTIONS,
                "invalid execution classification")
        require(type(row["required"]) is bool and type(row["details"]) is dict, "invalid check requirement/details")
        require(type(row["logs"]) is list and 1 <= len(row["logs"]) <= 16, "check requires bounded actual evidence logs")
        if row["required"] and row["exit_status"] == 0:
            require(row["classification"] != "unsupported" and row["execution"] != "none",
                    "unexecuted required check cannot pass")
        observed = []
        paths = set()
        for name in row["logs"]:
            require(type(name) is str and 1 <= len(name) <= 512, "invalid evidence path")
            relative = Path(name)
            require(not relative.is_absolute() and ".." not in relative.parts and name not in paths,
                    "unsafe or duplicate evidence path")
            paths.add(name)
            target = artifacts / relative
            require(target.resolve(strict=True).is_relative_to(artifacts), "evidence escaped its owned run directory")
            for component in (target, *target.parents):
                if component == artifacts:
                    break
                require(not component.is_symlink(), "symlink evidence path is forbidden")
            entry = target.stat(follow_symlinks=False)
            identity = (entry.st_dev, entry.st_ino, entry.st_size, entry.st_mtime_ns, entry.st_ctime_ns)
            require(target not in snapshots or snapshots[target] == identity, "evidence changed between check references")
            if identity in physical:
                item = physical[identity]
            else:
                item = digest(target, min(64 * 1024**2, remaining))
                after = target.stat(follow_symlinks=False)
                require(identity == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns),
                        "evidence changed while publishing")
                physical[identity] = item
                remaining -= item["bytes"]
            snapshots[target] = identity
            observed.append({"path": name, **item})
        result.append({**row, "logs": observed})
    return result


def publish(path, value):
    require(not path.exists() and not path.is_symlink(), "refusing to overwrite existing gate evidence")
    payload = (json.dumps(value, indent=2, allow_nan=False) + "\n").encode()
    require(len(payload) <= 2 * 1024**2, "accelerator report exceeds its ceiling")
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(prefix=".accelerator-report-", suffix=".tmp", dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            output.write(payload)
            output.flush()
        os.link(temporary, path)
        temporary.unlink()
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def retain_evidence(artifacts, report, base):
    """Keep only bounded evidence, not native build/cache intermediates."""
    require(base.is_absolute() and base.is_relative_to(ROOT / ".seen"), "retention root is not project-local")
    for component in (base, *base.parents):
        if component == ROOT:
            break
        require(not component.is_symlink(), "symlink retention directory is forbidden")
    base.mkdir(mode=0o700, parents=True, exist_ok=True)
    index_path = base / "index.json"
    index_pending = base / ".index.pending"

    def save_index(entries):
        require(not index_pending.is_symlink(), "symlink retention index is forbidden")
        index_pending.unlink(missing_ok=True)
        with index_pending.open("xb") as stream:
            stream.write((json.dumps({"schema": "seen-accelerator-retention-v1",
                                     "source_root": str(ROOT), "entries": entries}, allow_nan=False) + "\n").encode())
        os.replace(index_pending, index_path)

    def remove_owned(entry):
        paths = [base / entry["name"]]
        if entry["state"] == "pending":
            paths.append(base / (entry["name"] + ".partial"))
        for path in paths:
            if not path.exists() and not path.is_symlink():
                continue
            require(not path.is_symlink() and stat.S_ISREG(path.stat(follow_symlinks=False).st_mode),
                    "retention entry is no longer an owned regular file")
            if entry["state"] != "pending":
                require(digest(path, MAX_ARCHIVE) == {"sha256": entry["sha256"], "bytes": entry["bytes"]},
                        "retained evidence changed; refusing to delete it")
            path.unlink()

    original_file_limits = resource.getrlimit(resource.RLIMIT_FSIZE)
    require(original_file_limits[1] == resource.RLIM_INFINITY or original_file_limits[1] >= MAX_ARCHIVE,
            "host hard file limit cannot accommodate the bounded evidence archive")
    descriptor = os.open(base / ".lock", os.O_CREAT | os.O_RDWR | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    try:
        # Raw producers retain their smaller soft limit. Only this bounded
        # writer uses the archive ceiling; never raise a host hard limit.
        resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_ARCHIVE, original_file_limits[1]))
        require(stat.S_ISREG(os.fstat(descriptor).st_mode), "retention lock is not regular")
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        entries = []
        if index_path.exists() or index_path.is_symlink():
            index = json.loads(read_bounded(index_path, 32 * 1024).decode(),
                               object_pairs_hook=unique_object, parse_constant=invalid_constant)
            require(type(index) is dict and set(index) == {"schema", "source_root", "entries"} and
                    index["schema"] == "seen-accelerator-retention-v1" and index["source_root"] == str(ROOT),
                    "foreign retention index")
            entries = index["entries"]
            require(type(entries) is list and len(entries) <= 16, "retention index is not bounded")
            names = set()
            for entry in entries:
                require(type(entry) is dict and set(entry) == {"name", "phase", "state", "bytes", "sha256"},
                        "invalid retention entry")
                match = BUNDLE_NAME.fullmatch(entry["name"]) if type(entry["name"]) is str else None
                require(match is not None and entry["phase"] == match[1] and entry["name"] not in names and
                        type(entry["state"]) is str and entry["state"] in {"pending", "complete", "prune"} and
                        type(entry["bytes"]) is int and 0 <= entry["bytes"] <= MAX_ARCHIVE and
                        type(entry["sha256"]) is str and
                        (entry["state"] == "pending" or re.fullmatch(r"[0-9a-f]{64}", entry["sha256"])),
                        "unsafe retention entry")
                names.add(entry["name"])
            for entry in entries:
                if entry["state"] != "complete":
                    remove_owned(entry)
            entries = [entry for entry in entries if entry["state"] == "complete"]
        # Reserve names before creating bytes. Interrupted publications remain
        # indexed and are reclaimed on the next invocation under this lock.
        space = os.statvfs(base)
        require(space.f_bavail * space.f_frsize >= MAX_ARCHIVE + 64 * 1024**2, "insufficient free space for bounded evidence")
        name = f"gate-{report['phase']}-{uuid.uuid4().hex}.tar.gz"
        entry = {"name": name, "phase": report["phase"], "state": "pending", "bytes": 0, "sha256": ""}
        entries.append(entry)
        save_index(entries)
        destination = base / name
        partial = base / (name + ".partial")
        report["retained_evidence"] = {"archive": str(destination.relative_to(ROOT)), "artifact_prefix": "logs",
                                       "source_root": str(ROOT), "retained_per_phase": RETAINED_PER_PHASE,
                                       "maximum_archive_bytes": MAX_ARCHIVE}
        payload = (json.dumps(report, indent=2, allow_nan=False) + "\n").encode()
        require(len(payload) <= 2 * 1024**2, "accelerator report exceeds its ceiling")
        expected = {}
        for row in report["checks"]:
            for item in row["logs"]:
                previous = expected.setdefault(item["path"], item)
                require(previous == item, "log snapshot differs between checks")
        shared = artifacts / "shared-foundation"
        if shared.exists() or shared.is_symlink():
            require(shared.is_dir() and not shared.is_symlink(), "shared foundation is not an owned directory")
            with os.scandir(shared) as directory:
                files = [Path(entry.path) for entry in islice(directory, 33)]
            require(len(files) <= 32, "shared foundation file count exceeds its ceiling")
            files.sort()
            for path in files:
                require(path.is_file() and not path.is_symlink(), "shared foundation entry is not regular")
                relative = str(path.relative_to(artifacts))
                expected.setdefault(relative, {"path": relative, **digest(path, 32 * 1024**2)})
        physical = {}
        remaining = MAX_LOGS
        try:
            with partial.open("xb") as stream:
                with tarfile.open(fileobj=stream, mode="w:gz", compresslevel=6) as archive:
                    info = tarfile.TarInfo("evidence.json")
                    info.size = len(payload)
                    info.mode = 0o600
                    archive.addfile(info, io.BytesIO(payload))
                    for relative, item in expected.items():
                        path = artifacts / relative
                        before = path.stat(follow_symlinks=False)
                        identity = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns)
                        info = tarfile.TarInfo("logs/" + relative)
                        info.mode = 0o600
                        if identity in physical:
                            first, prior = physical[identity]
                            require(item["sha256"] == prior["sha256"] and item["bytes"] == prior["bytes"],
                                    "hard-linked evidence snapshots differ")
                            info.type = tarfile.LNKTYPE
                            info.linkname = first
                            archive.addfile(info)
                        else:
                            content = read_bounded(path, min(64 * 1024**2, remaining))
                            after = path.stat(follow_symlinks=False)
                            require(identity == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns) and
                                    len(content) == item["bytes"] and hashlib.sha256(content).hexdigest() == item["sha256"],
                                    "evidence changed before retention")
                            remaining -= len(content)
                            info.size = len(content)
                            archive.addfile(info, io.BytesIO(content))
                            physical[identity] = (info.name, item)
                stream.flush()
                require(os.fstat(stream.fileno()).st_size <= MAX_ARCHIVE, "evidence archive exceeds its ceiling")
            os.link(partial, destination)
            partial.unlink()
            entry.update(digest(destination, MAX_ARCHIVE))
            entry["state"] = "complete"
            phase_entries = [value for value in entries if value["phase"] == entry["phase"] and value["state"] == "complete"]
            for obsolete in phase_entries[:-RETAINED_PER_PHASE]:
                obsolete["state"] = "prune"
            save_index(entries)
            for obsolete in entries:
                if obsolete["state"] == "prune":
                    remove_owned(obsolete)
            save_index([value for value in entries if value["state"] == "complete"])
        except BaseException:
            # A pending index survives abrupt termination as an ownership record.
            # Ordinary failures reclaim only this invocation's reserved names.
            if entry["state"] == "pending":
                remove_owned(entry)
                save_index([value for value in entries if value is not entry])
            raise
        return destination
    finally:
        try:
            os.close(descriptor)
        finally:
            resource.setrlimit(resource.RLIMIT_FSIZE, original_file_limits)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices="abcd", required=True)
    for name in ("compiler", "root", "artifacts", "checks", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    try:
        require(args.root.resolve(strict=True) == ROOT, "gate source root does not match the executing helper")
        parent = Path(os.environ["SEEN_ARTIFACT_ROOT"]).resolve(strict=True)
        artifacts = args.artifacts.resolve(strict=True)
        require(artifacts.is_relative_to(parent) and artifacts.is_dir(), "gate artifacts are not project-local")
        require(args.checks.resolve(strict=True).is_relative_to(artifacts) and args.checks.stat().st_size <= MAX_DOCUMENT,
                "check manifest is not bounded/local")
        output = args.output.absolute()
        require(output.parent.resolve(strict=True).is_relative_to(artifacts), "report output escaped its owned run directory")
        checks = validate_checks(json.loads(read_bounded(args.checks, MAX_DOCUMENT).decode("utf-8"),
                                           object_pairs_hook=unique_object, parse_constant=invalid_constant), artifacts)
        scope = containment()
        report = {"schema": "seen-accelerator-gate-evidence-v1", "phase": args.phase,
                  "source": source_identity(), "compiler": {"path": str(args.compiler), **digest(args.compiler, 256 * 1024**2)},
                  "containment": scope, "checks": checks, "phase_checks_passed":
                  all(not row["required"] or row["exit_status"] == 0 for row in checks),
                  "production_certified": False,
                  "command": ["bash", f"tests/misc_root_tests/seen_p0_accel_gate_{args.phase}.sh"]}
        retain_evidence(artifacts, report, ROOT / ".seen/accelerator-gate-evidence")
        publish(output, report)
        print(f"accelerator-gate-{args.phase}: phase_checks_passed={report['phase_checks_passed']}; source_clean={report['source']['clean']}; production_certified=false; retained_evidence={report['retained_evidence']['archive']}")
        return 0 if report["phase_checks_passed"] else 1
    except (ValueError, OSError, KeyError, StopIteration, RecursionError, subprocess.SubprocessError) as error:
        print(f"accelerator-gate: invalid evidence: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
