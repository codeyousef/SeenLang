#!/usr/bin/env python3
"""Supervise an attested fixture and collect real Linux process observations."""
import argparse
import hashlib
import json
import os
import platform
import selectors
import signal
import subprocess
import time
from pathlib import Path

from io_overlap_evidence import STAGES, decode, require, validate_checkpoint, validate_report

ROOT = Path(__file__).resolve().parent.parent


def digest(path):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 256 * 1024**2,
            "source/toolchain artifact is missing or exceeds ceiling")
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def containment():
    # The wrapper verifies the transient unit, aggregate budget and namespace.
    subprocess.run(["bash", str(ROOT / "scripts/run_in_hard_memory_scope.sh"), "--verify-only"],
                   check=True, timeout=20)
    relative = next(line[3:] for line in Path("/proc/self/cgroup").read_text().splitlines() if line.startswith("0::"))
    require(relative.startswith("/") and ".." not in Path(relative).parts, "invalid cgroup membership")
    group = Path("/sys/fs/cgroup") / relative.lstrip("/")
    values = {key: int((group / file).read_text()) for key, file in {
        "memory_max_bytes": "memory.max", "swap_max_bytes": "memory.swap.max",
        "pids_max": "pids.max", "oom_group": "memory.oom.group",
    }.items()}
    require(values["memory_max_bytes"] > 0 and values["swap_max_bytes"] == 0
            and 1 <= values["pids_max"] <= 24 and values["oom_group"] == 1, "scope readback differs")
    return {"path": relative, **values}


def blocked_reads(pid, descriptors):
    require(len(descriptors) == 2 and len(set(descriptors)) == 2, "invalid pipe reader identities")
    for descriptor in descriptors:
        require(os.readlink(f"/proc/{pid}/fd/{descriptor}").startswith("pipe:["), "reader is not a kernel pipe")
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        observations = []
        for task in sorted(Path(f"/proc/{pid}/task").iterdir()):
            try:
                fields = (task / "syscall").read_text().split()
            except FileNotFoundError:
                continue
            # Permission errors are deliberately fatal: a clock interval alone
            # does not prove that the workers entered a kernel I/O operation.
            if len(fields) >= 4 and fields[0] == "0":
                descriptor = int(fields[1], 0)
                if descriptor in descriptors:
                    require(int(fields[3], 0) == 16, "kernel read transfer length differs")
                    observations.append({"tid": int(task.name), "fd": descriptor, "syscall": 0,
                                         "observed_ns": time.monotonic_ns()})
        if len(observations) == 2 and {row["fd"] for row in observations} == set(descriptors):
            return observations
        time.sleep(0.01)
    raise ValueError("kernel read observations unavailable; overlap is not certified")


def sample(event, pid):
    fields = {}
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        if line.startswith(("VmRSS:", "VmHWM:")):
            key, number, unit = line.split()
            require(unit == "kB", "unexpected process resource unit")
            fields[key] = int(number) * 1024
    reads = blocked_reads(pid, [event["reader_a"], event["reader_b"]]) if event["stage"] in ("blocked", "computed") else []
    return {"checkpoint": event, "rss_bytes": fields["VmRSS:"], "hwm_bytes": fields["VmHWM:"],
            "fd_count": len(list(Path(f"/proc/{pid}/fd").iterdir())), "blocked_reads": reads}


def collect(command):
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True, bufsize=0)
    events = []; buffer = b""; total = 0; deadline = time.monotonic() + 120
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    expected_group = Path("/proc/self/cgroup").read_text()
    try:
        while selector.get_map():
            require(time.monotonic() < deadline, "fixture exceeded 120-second ceiling")
            for key, _ in selector.select(timeout=0.25):
                chunk = os.read(key.fileobj.fileno(), 16384)
                if not chunk:
                    selector.unregister(key.fileobj); continue
                total += len(chunk); require(total <= 1_048_576, "fixture output exceeded byte ceiling")
                if key.data == "stderr":
                    os.write(2, chunk); continue
                buffer += chunk; require(len(buffer) <= 16384, "checkpoint line exceeded byte ceiling")
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    require(len(events) < len(STAGES), "unexpected extra fixture output")
                    event = decode(line.decode("utf-8"))
                    # The ASan wrapper execs the fixture: no child-supplied PID
                    # can redirect observations to an unrelated process.
                    validate_checkpoint(event, STAGES[len(events)], process.pid)
                    require(Path(f"/proc/{process.pid}/cgroup").read_text() == expected_group,
                            "fixture escaped aggregate scope")
                    events.append(sample(event, process.pid))
                    process.stdin.write(b"G"); process.stdin.flush()
        require(not buffer and len(events) == len(STAGES), "fixture ended without complete evidence")
        require(process.wait(timeout=max(0.1, deadline - time.monotonic())) == 0, "fixture failed")
        return events
    finally:
        selector.close()
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=5)
        for stream in (process.stdin, process.stdout, process.stderr):
            stream.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--profile", choices=("fast", "release", "undefined", "address"), required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    require(platform.system() == "Linux" and platform.machine() == "x86_64", "kernel observer requires Linux x86-64")
    require(os.environ.get("SEEN_CAPPED_REGRESSION_ACTIVE") == "1", "attested regression wrapper is required")
    artifact_root = Path(os.environ["SEEN_ARTIFACT_ROOT"]).resolve(strict=True)
    parent = args.output.parent.resolve(strict=True)
    require(parent.is_relative_to(artifact_root) and parent != artifact_root
            and not args.output.exists() and not args.output.is_symlink(), "evidence output must be new and inside fixture artifacts")
    require(args.executable.resolve(strict=True).parent == parent, "executable is outside fixture artifacts")
    compiler = args.compiler.resolve(strict=True)
    require(compiler == Path(os.environ["SEEN_CAPPED_REGRESSION_COMPILER"]).resolve(strict=True),
            "compiler differs from attested regression compiler")
    require(args.source.resolve(strict=True) == ROOT / "tests/cross_platform_asynchronous_i_o/io_overlap.seen",
            "source differs from the overlap witness")
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    require(1 <= len(command) <= 16, "missing or oversized fixture command")
    group = containment()
    hashes = {"source_sha256": digest(args.source), "executable_sha256": digest(args.executable), "compiler_sha256": digest(compiler)}
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, timeout=10).strip()
    subprocess.run(["git", "diff", "--quiet", "HEAD", "--"], cwd=ROOT, check=True, timeout=10)
    cpu = next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                if line.startswith("model name")), platform.machine())
    report = {"schema": "seen-io-overlap-v1", "target": "linux-x86_64", "backend": "explicit-worker-kernel-pipes",
              "profile": args.profile, "classification": "verified", "storage_overlap_verified": False,
              "source_commit": commit, **hashes, "command": command,
              "machine": {"system": platform.system(), "release": platform.release(), "machine": platform.machine(), "cpu": cpu},
              "cgroup": group, "rss_growth_limit_bytes": min((512 if args.profile == "address" else 64) * 1024**2,
                                                             group["memory_max_bytes"] // 8),
              "stages": collect(command)}
    validate_report(report)
    with args.output.open("x") as output:
        json.dump(report, output, sort_keys=True, separators=(",", ":")); output.write("\n")
    print("PASS: retained verified kernel-pipe overlap and resource evidence")


if __name__ == "__main__":
    main()
