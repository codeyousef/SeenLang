#!/usr/bin/env python3
"""Bounded, process-group-owned observer for the real native async soak."""
import argparse
import json
import os
from pathlib import Path
import selectors
import signal
import stat
import subprocess
import sys
import time

SCHEMA = "seen-async-stress-soak-v1"
LIMIT = 2 * 1024 * 1024
LINE_LIMIT = 4096
COUNTERS = ("iterations", "cpu_tasks", "io_completions", "gpu_completions", "gpu_callbacks")
RESOURCES = ("memory_bytes", "threads", "mutexes", "conditions", "atomics", "readiness_handles")
OS_FIELDS = ("fd_count", "thread_count", "child_count", "rss_kb", "hwm_kb")


class ContractError(Exception):
    pass


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ContractError("duplicate JSON key: " + key)
        result[key] = value
    return result


def integer(record, key, minimum=0):
    value = record.get(key)
    if type(value) is not int or value < minimum or value > 9223372036854775807:
        raise ContractError("invalid integral field: " + key)
    return value


def encoded(record):
    data = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode()
    if len(data) > LINE_LIMIT:
        raise ContractError("observer record exceeds 4 KiB")
    return data


class BoundedFile:
    def __init__(self, path):
        self.stream = path.open("xb")
        self.size = 0

    def write(self, data):
        if self.size + len(data) > LIMIT:
            raise ContractError("2 MiB artifact limit exceeded")
        self.stream.write(data)
        self.stream.flush()
        self.size += len(data)

    def close(self):
        self.stream.close()


def observe(pid):
    """Read the owned process only; never synthesize post-reap zeroes."""
    try:
        root = Path("/proc") / str(pid)
        text = (root / "status").read_text()
        if len(text) > 65536:
            raise ContractError("oversized /proc status")
        values = {}
        for line in text.splitlines():
            key, _, value = line.partition(":")
            if key in ("VmRSS", "VmHWM", "Threads"):
                words = value.split()
                if not words or not words[0].isdigit():
                    raise ContractError("invalid /proc status value")
                if key != "Threads" and words[1:] != ["kB"]:
                    raise ContractError("unexpected /proc memory unit")
                values[key] = int(words[0])
        tasks = list((root / "task").iterdir())
        if len(tasks) > 128:
            raise ContractError("unexpected task amplification")
        children = set()
        for task in tasks:
            data = (task / "children").read_text()
            if len(data) > 4096:
                raise ContractError("oversized /proc children list")
            children.update(int(item) for item in data.split())
        fds = list((root / "fd").iterdir())
        if len(fds) > 16384:
            raise ContractError("unexpected descriptor amplification")
        if not all(key in values for key in ("VmRSS", "VmHWM", "Threads")):
            return {"observation": "unavailable", "reason": "process exiting or memory metadata absent"}
        return {"observation": "observed", "fd_count": len(fds),
                "thread_count": values["Threads"], "child_count": len(children),
                "rss_kb": values["VmRSS"], "hwm_kb": values["VmHWM"]}
    except (OSError, ValueError) as error:
        return {"observation": "unavailable", "reason": type(error).__name__}


def exited_unreaped(pid):
    # WNOWAIT keeps the leader identity reserved through group cleanup.
    return os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None


def group_members(pgid):
    members = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            text = (entry / "stat").read_text()
            fields = text[text.rfind(")") + 2:].split()
            if int(fields[2]) == pgid:
                members.append(int(entry.name))
        except (OSError, ValueError, IndexError):
            continue
    return members


def send_group(pgid, signum):
    try:
        os.killpg(pgid, signum)
    except ProcessLookupError:
        pass


class Protocol:
    def __init__(self, seconds, device_class):
        self.seconds = seconds
        self.device_class = device_class
        self.phase = None
        self.previous = None
        self.uuid = None
        self.last_heartbeat_ns = None
        self.passed = None

    def consume(self, line, wall_ns):
        if not line.lstrip().startswith(b"{"):
            return None
        try:
            record = json.loads(line, object_pairs_hook=unique_object,
                                parse_constant=lambda _: (_ for _ in ()).throw(ContractError("nonfinite JSON")))
        except (ValueError, UnicodeError, RecursionError) as error:
            raise ContractError("invalid JSON record") from error
        if not isinstance(record, dict) or record.get("schema") != SCHEMA:
            raise ContractError("unexpected JSON schema")
        phase = record.get("phase")
        if phase == "failed":
            raise ContractError("native reported failed phase")
        if phase not in ("begin", "heartbeat", "passed"):
            raise ContractError("invalid native phase")
        if self.phase == "passed" or (self.phase is None and phase != "begin") or (self.phase is not None and phase == "begin"):
            raise ContractError("invalid native phase ordering")
        if integer(record, "planned_seconds", 1) != self.seconds:
            raise ContractError("native duration differs from exact argv")
        elapsed = integer(record, "elapsed_ns")
        if elapsed > wall_ns + 100000000:
            raise ContractError("native elapsed exceeds observed process lifetime")
        for key in COUNTERS:
            integer(record, key)
        if record["device_class"] != self.device_class:
            raise ContractError("native device class differs from exact argv")
        uuid = record.get("device_uuid")
        if not isinstance(uuid, str) or not uuid or len(uuid) > 128:
            raise ContractError("invalid device UUID")
        if self.uuid is not None and uuid != self.uuid:
            raise ContractError("device identity changed")
        self.uuid = uuid
        verified = record.get("hardware_execution_verified")
        if type(verified) is not bool or verified != (self.device_class == "rtx4090"):
            raise ContractError("invalid hardware classification")
        for key in ("production_certified", "performance_regression_verified", "tsan_verified"):
            if record.get(key) is not False:
                raise ContractError("unsupported certification claim: " + key)
        if self.previous:
            for key in ("elapsed_ns",) + COUNTERS:
                if record[key] < self.previous[key]:
                    raise ContractError("native counter regressed: " + key)
        iterations = record["iterations"]
        for key, multiplier in (("cpu_tasks", 39), ("io_completions", 2), ("gpu_completions", 1), ("gpu_callbacks", 1)):
            if record[key] != iterations * multiplier:
                raise ContractError("inconsistent completed-work counters: " + key)
        if phase == "begin":
            if any(record[key] != 0 for key in COUNTERS):
                raise ContractError("begin counters include untimed work")
            if integer(record, "warmup_count") != 5 or integer(record, "measured_count") != 30:
                raise ContractError("native warmup/sample count differs from full-workload contract")
            samples = record.get("raw_mixed_nanoseconds")
            if not isinstance(samples, list) or len(samples) != 30 or any(type(value) is not int or not 1 <= value <= 9223372036854775807 for value in samples):
                raise ContractError("missing typed actual raw mixed-workload samples")
            for resource in RESOURCES:
                integer(record, "baseline_" + resource)
        if phase == "heartbeat":
            if self.last_heartbeat_ns is not None and wall_ns - self.last_heartbeat_ns < 59000000000:
                raise ContractError("native heartbeat exceeds minute cadence")
            self.last_heartbeat_ns = wall_ns
        if phase == "passed":
            if elapsed < self.seconds * 1000000000 or wall_ns < self.seconds * 1000000000:
                raise ContractError("native completed before requested monotonic duration")
            if iterations == 0 or record.get("cleanup_verified") is not True:
                raise ContractError("native did not verify complete workload cleanup")
            for resource in RESOURCES:
                baseline = integer(record, "baseline_" + resource)
                final = integer(record, "final_" + resource)
                if baseline != self.previous_baseline[resource]:
                    raise ContractError("native resource baseline changed")
                evidence = integer(record, "report_evidence_bytes") if resource == "memory_bytes" else 0
                if final != baseline + evidence:
                    raise ContractError("native resource cleanup imbalance: " + resource)
            self.passed = record
        if phase == "begin":
            self.previous_baseline = {key: record["baseline_" + key] for key in RESOURCES}
        self.phase = phase
        self.previous = record
        return phase


def supervise(argv, seconds, device_class, output, label="soak", rejection=None):
    capture = BoundedFile(output / (label + ".log"))
    metrics = BoundedFile(output / (label + ".metrics.jsonl"))
    protocol = Protocol(seconds, device_class)
    selector = selectors.DefaultSelector()
    child = None
    interrupted = []
    old_handlers = {}
    error = None
    raw_status = None
    baseline = None
    last = None
    peak = None
    last_observation = None
    buffer = bytearray()
    diagnostic = False
    saw_json = False
    started = time.monotonic_ns()
    # 35 full mixed warmup/sample rounds have 10s checked native deadlines.
    # Allow bounded setup as well, independently of the requested interval.
    deadline = started + (10 if rejection else seconds + 480) * 1000000000
    next_observation = started
    next_metric = started + 60000000000
    exit_seen = None
    last_protocol_ns = None
    cleanup_verified = False

    def record_observation():
        nonlocal baseline, last, peak, last_observation
        try:
            file_state = Path(argv[2]).lstat()
        except FileNotFoundError:
            file_state = None
        if file_state is not None and (not stat.S_ISREG(file_state.st_mode) or file_state.st_size > 8192):
            raise ContractError("native direct-I/O file exceeded fixed 8 KiB contract")
        snapshot = observe(child.pid)
        last_observation = snapshot
        if snapshot["observation"] == "observed":
            last = snapshot
            if snapshot["child_count"]:
                raise ContractError("native created unexpected child processes")
            if peak is None:
                peak = dict(snapshot)
            else:
                for key in OS_FIELDS:
                    peak[key] = max(peak[key], snapshot[key])
        return snapshot

    def metric(phase):
        metrics.write(encoded({"schema": "seen-async-stress-soak-monitor-v1", "phase": phase,
                               "observed_elapsed_ns": time.monotonic_ns() - started,
                               "process": last_observation}))

    def consume(line):
        nonlocal baseline, peak, diagnostic, saw_json, last_protocol_ns
        if len(line) + 1 > LINE_LIMIT:
            raise ContractError("native output line exceeds 4 KiB")
        if rejection:
            if b"async.invalid" in line or b"async.limit" in line:
                diagnostic = True
            if line.lstrip().startswith(b"{"):
                saw_json = True
            return
        phase = protocol.consume(line, time.monotonic_ns() - started)
        if phase in ("begin", "heartbeat"):
            last_protocol_ns = time.monotonic_ns()
        if phase == "begin":
            snapshot = record_observation()
            if snapshot["observation"] != "observed":
                raise ContractError("steady-state /proc baseline not observed")
            baseline = dict(snapshot)
            peak = dict(snapshot)
            metric("begin")
        # Forward only the bounded native stream; no unbounded communicate().
        sys.stdout.buffer.write(line + b"\n")
        sys.stdout.buffer.flush()

    try:
        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            old_handlers[signum] = signal.signal(signum, lambda sig, _: interrupted.append(sig) if not interrupted else None)
        child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, start_new_session=True, bufsize=0)
        os.set_blocking(child.stdout.fileno(), False)
        selector.register(child.stdout, selectors.EVENT_READ)
        while True:
            now = time.monotonic_ns()
            if interrupted:
                raise ContractError("observer interrupted by signal " + str(interrupted[0]))
            if now >= deadline:
                raise ContractError("finite native deadline exceeded")
            if now >= next_observation:
                record_observation()
                next_observation = now + 1000000000
            if now >= next_metric:
                metric("heartbeat")
                if not rejection:
                    sys.stdout.buffer.write(encoded({"schema": "seen-async-stress-soak-monitor-v1", "phase": "heartbeat",
                                                     "observed_elapsed_ns": now - started, "process": last_observation}))
                    sys.stdout.buffer.flush()
                next_metric = now + 60000000000
            if not rejection and protocol.phase is None and now - started > 420000000000:
                raise ContractError("native steady-state begin deadline exceeded")
            if not rejection and protocol.phase != "passed" and last_protocol_ns is not None and now - last_protocol_ns > 75000000000:
                raise ContractError("native minute heartbeat deadline exceeded")
            if exited_unreaped(child.pid):
                if exit_seen is None:
                    exit_seen = now
                if not selector.get_map():
                    break
                if now - exit_seen > 5000000000:
                    raise ContractError("pipe remained open after native exit")
            for key, _ in selector.select(0.1):
                data = os.read(key.fd, LINE_LIMIT)
                if not data:
                    selector.unregister(key.fileobj)
                    if buffer:
                        consume(bytes(buffer))
                        buffer.clear()
                    continue
                capture.write(data)
                buffer.extend(data)
                while b"\n" in buffer:
                    line, _, rest = buffer.partition(b"\n")
                    buffer[:] = rest
                    consume(bytes(line))
                if len(buffer) > LINE_LIMIT:
                    raise ContractError("unterminated native line exceeds 4 KiB")
        if rejection:
            if not diagnostic or saw_json:
                raise ContractError("invalid native argv did not fail before workload setup")
        elif protocol.passed is None or baseline is None:
            raise ContractError("native exited without verified completion")
        if not rejection:
            file_state = Path(argv[2]).lstat()
            if not stat.S_ISREG(file_state.st_mode) or file_state.st_size != 8192:
                raise ContractError("completed direct-I/O file is not exactly 8 KiB")
    except (ContractError, OSError, KeyError) as failure:
        error = str(failure)
    finally:
        if child is not None:
            try:
                # The unreaped session leader reserves the PGID until all owned
                # descendants have been cancelled. No global pkill/tree killer.
                members = group_members(child.pid)
                live_descendants = [pid for pid in members if pid != child.pid]
                if live_descendants:
                    error = error or "unexpected descendants at native exit"
                if not exited_unreaped(child.pid) or live_descendants:
                    send_group(child.pid, signal.SIGTERM)
                    end = time.monotonic() + 5
                    while time.monotonic() < end:
                        if exited_unreaped(child.pid) and not [pid for pid in group_members(child.pid) if pid != child.pid]:
                            break
                        time.sleep(0.05)
                    send_group(child.pid, signal.SIGKILL)
                    end = time.monotonic() + 5
                    while time.monotonic() < end and [pid for pid in group_members(child.pid) if pid != child.pid]:
                        time.sleep(0.05)
                remaining = [pid for pid in group_members(child.pid) if pid != child.pid]
                if remaining:
                    error = error or "owned descendants survived bounded group cleanup"
                raw_status = child.wait(timeout=5)
                cleanup_verified = not remaining
            except (OSError, subprocess.TimeoutExpired) as failure:
                error = error or "bounded process-group cleanup failed: " + str(failure)
                send_group(child.pid, signal.SIGKILL)
                try:
                    raw_status = child.wait(timeout=5)
                except (OSError, subprocess.TimeoutExpired):
                    raw_status = None
            if child.stdout is not None:
                child.stdout.close()
        selector.close()
        for signum, handler in old_handlers.items():
            signal.signal(signum, handler)
        if not rejection and raw_status != 0:
            error = error or "native exited nonzero after protocol"
        if rejection and (raw_status is None or not 1 <= raw_status <= 125):
            error = error or "invalid argument did not produce a normal nonzero native exit"
        summary = {"schema": "seen-async-stress-soak-monitor-v1", "phase": "failed" if error else "passed",
                   "scope": "component-only", "planned_seconds": seconds,
                   "native_argv": argv, "native_pid": child.pid if child is not None else None,
                   "observed_elapsed_ns": time.monotonic_ns() - started,
                   "device_class": device_class, "native_returncode": raw_status,
                   "process_group_cleanup_verified": cleanup_verified,
                   "baseline": baseline, "peak": peak, "last": last,
                   "os_peak_scope": "steady-state-begin-through-last-observation" if baseline else "startup-through-last-observation",
                   "after_reap": {"observation": "not-observed-after-reap"},
                   "production_certified": False, "performance_regression_verified": False,
                   "tsan_verified": False, "physical_24h_observed": bool(not error and not rejection and seconds == 86400 and device_class == "rtx4090"),
                   "error": error}
        try:
            metrics.write(encoded(summary))
            sys.stdout.buffer.write(encoded(summary))
            sys.stdout.buffer.flush()
        except (ContractError, OSError):
            error = error or "bounded summary publication failed"
        capture.close()
        metrics.close()
    if raw_status is None:
        return 70
    if raw_status != 0:
        return 128 - raw_status if raw_status < 0 else raw_status
    if rejection:
        return 70  # Rejection must be an actual nonzero native exit.
    return 70 if error else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=int, required=True)
    parser.add_argument("--device-class", choices=("rtx4090", "software-reference"), required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--check-boundaries", action="store_true")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    argv = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not 1 <= args.seconds <= 86400 or len(argv) != 6 or argv[1] != str(args.seconds) or argv[5] != args.device_class:
        parser.error("exact native argv must be PROGRAM SECONDS IO_PATH SHADER_SPV REFLECTION_JSON DEVICE_CLASS")
    if not args.output_dir.is_dir() or not os.path.isabs(argv[0]):
        parser.error("existing output directory and absolute native program required")
    if args.check_boundaries:
        cases = (("zero", "0", "software-reference"), ("negative", "-1", "software-reference"),
                 ("overflow", "999999999999999999999999999999999999", "software-reference"),
                 ("over-limit", "86401", "software-reference"), ("malformed", "1x", "software-reference"),
                 ("empty", "", "software-reference"), ("class", "1", "invalid-device"))
        for name, duration, device in cases:
            invalid = list(argv)
            io_path = args.output_dir / ("invalid-" + name + ".bin")
            invalid[1], invalid[2], invalid[5] = duration, str(io_path), device
            if io_path.exists():
                parser.error("invalid-argv side-effect sentinel already exists")
            status = supervise(invalid, 1, device, args.output_dir, "invalid-" + name, rejection=True)
            # supervise preserves nonzero native status; inspect bounded evidence,
            # not source text, and reject signals/timeouts/crashes as invalid proof.
            records = (args.output_dir / ("invalid-" + name + ".metrics.jsonl")).read_text().splitlines()
            result = json.loads(records[-1], object_pairs_hook=unique_object)
            if status == 0 or result["native_returncode"] is None or not 1 <= result["native_returncode"] <= 125 or result["error"] or io_path.exists():
                print("FAIL: real native invalid boundary " + name, file=sys.stderr)
                return 70
        print("PASS: real native invalid duration/class boundaries before I/O setup")
    return supervise(argv, args.seconds, args.device_class, args.output_dir)


if __name__ == "__main__":
    sys.exit(main())
