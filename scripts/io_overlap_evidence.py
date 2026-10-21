#!/usr/bin/env python3
"""Strict, data-only validation of the Linux kernel-pipe overlap witness."""
import argparse
import json
from functools import lru_cache
from pathlib import Path

STAGES = ("start", "setup", "blocked", "computed", "drained", "soak", "closed")
COUNTS = ("accepted", "submitted", "terminal", "delivered", "transferred_bytes")
FIELDS = {
    "schema", "stage", "pid", "managed_bytes", "reader_a", "reader_b",
    "compute_start_ns", "compute_end_ns", "read_a_start_ns", "read_a_end_ns",
    "read_b_start_ns", "read_b_end_ns", "checksum", *COUNTS, "peak_active",
    "peak_submitted", "peak_owned_bytes", "peak_metadata_bytes", "saturated",
}
MAX_JSON_BYTES = 1_048_576


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, f"duplicate JSON field: {key}")
        result[key] = value
    return result


def decode(text):
    require(len(text.encode()) <= MAX_JSON_BYTES, "evidence exceeds byte ceiling")
    return json.loads(text, object_pairs_hook=unique_pairs,
                      parse_constant=lambda _: require(False, "non-finite JSON number"))


def integer(value, minimum=0, maximum=(1 << 63) - 1):
    return type(value) is int and minimum <= value <= maximum


@lru_cache(maxsize=8)
def compute_checksum(pid):
    checksum = pid
    for _ in range(1_000_000):
        checksum = (checksum * 1664525 + 1013904223) % 2147483647
    return checksum


def validate_checkpoint(event, stage, pid):
    require(type(event) is dict and set(event) == FIELDS, "checkpoint fields differ from schema")
    require(event["schema"] == "seen-io-checkpoint-v1" and event["stage"] == stage, "unexpected checkpoint")
    require(integer(event["pid"], 1, 1 << 30) and event["pid"] == pid, "checkpoint process identity differs")
    require(event["saturated"] is False, "saturated counters cannot certify lifecycle")
    for field in FIELDS - {"schema", "stage", "saturated", "pid"}:
        require(integer(event[field], -1 if field.startswith("reader_") else 0), f"invalid {field}")
    require(event["managed_bytes"] > 0, "managed allocation measurement is missing")
    index = STAGES.index(stage)
    count = 0 if index < 2 else 2 if index < 5 else 20002
    finished = count if index >= 4 else 0
    require(tuple(event[key] for key in COUNTS) == (count, count, finished, finished, finished * 16),
            "lifecycle counters do not certify exactly-once completion")
    require(event["peak_active"] == (2 if count else 0) and event["peak_submitted"] == (2 if count else 0),
            "queue concurrency exceeds or misses the bounded witness")
    require(event["peak_owned_bytes"] == (128 if count else 0), "owned-buffer peak differs")
    require(0 < event["peak_metadata_bytes"] <= 8192 if count else event["peak_metadata_bytes"] == 0,
            "metadata peak exceeds queue budget")
    if index >= 2:
        require(event["reader_a"] >= 3 and event["reader_b"] >= 3 and event["reader_a"] != event["reader_b"],
                "reader descriptor identities are invalid")
    if index >= 3:
        require(0 < event["compute_start_ns"] < event["compute_end_ns"], "CPU interval is empty")
        require(event["checksum"] == compute_checksum(pid), "CPU work checksum differs")
    if index >= 4:
        for reader in ("a", "b"):
            require(0 < event[f"read_{reader}_start_ns"] <= event["compute_start_ns"] < event["compute_end_ns"]
                    <= event[f"read_{reader}_end_ns"], "kernel I/O interval does not bracket CPU work")
    return event


def validate_report(report):
    require(type(report) is dict and set(report) == {
        "schema", "target", "backend", "profile", "classification", "storage_overlap_verified",
        "source_commit", "source_sha256", "executable_sha256", "compiler_sha256", "command",
        "machine", "cgroup", "rss_growth_limit_bytes", "stages",
    }, "report fields differ from schema")
    require(report["schema"] == "seen-io-overlap-v1" and report["target"] == "linux-x86_64", "unsupported evidence schema/target")
    require(report["backend"] == "explicit-worker-kernel-pipes" and report["classification"] == "verified", "backend witness is unverified")
    require(report["storage_overlap_verified"] is False, "pipe evidence cannot certify storage overlap")
    require(report["profile"] in ("fast", "release", "undefined", "address"), "unknown execution profile")
    for key, size in (("source_commit", 40), ("source_sha256", 64), ("executable_sha256", 64), ("compiler_sha256", 64)):
        value = report[key]
        require(type(value) is str and len(value) == size and set(value) <= set("0123456789abcdef"), "invalid source/toolchain identity")
    require(type(report["command"]) is list and 1 <= len(report["command"]) <= 16
            and all(type(x) is str and 0 < len(x) <= 4096 for x in report["command"]), "invalid execution command")
    machine = report["machine"]
    require(type(machine) is dict and set(machine) == {"system", "release", "machine", "cpu"}
            and all(type(x) is str and 0 < len(x) <= 4096 for x in machine.values()), "missing hardware identity")
    group = report["cgroup"]
    require(type(group) is dict and set(group) == {"path", "memory_max_bytes", "swap_max_bytes", "pids_max", "oom_group"}, "invalid cgroup evidence")
    require(type(group["path"]) is str and group["path"].startswith("/") and len(group["path"]) <= 4096, "invalid cgroup path")
    require(integer(group["memory_max_bytes"], 1) and integer(group["pids_max"], 1, 24)
            and type(group["swap_max_bytes"]) is int and group["swap_max_bytes"] == 0
            and type(group["oom_group"]) is int and group["oom_group"] == 1, "aggregate containment is missing")
    limit = report["rss_growth_limit_bytes"]
    require(integer(limit, 1, 512 * 1024**2) and limit <= group["memory_max_bytes"] // 8, "RSS growth budget exceeds scope")
    stages = report["stages"]
    require(type(stages) is list and len(stages) == len(STAGES), "missing lifecycle stages")
    pid = stages[0].get("checkpoint", {}).get("pid") if type(stages[0]) is dict else None
    for stage, sample in zip(STAGES, stages):
        require(type(sample) is dict and set(sample) == {"checkpoint", "rss_bytes", "hwm_bytes", "fd_count", "blocked_reads"}, "invalid resource sample")
        validate_checkpoint(sample["checkpoint"], stage, pid)
        require(integer(sample["rss_bytes"], 1) and integer(sample["hwm_bytes"], sample["rss_bytes"])
                and integer(sample["fd_count"], 3, 1024), "invalid process resource reading")
        reads = sample["blocked_reads"]
        if stage in ("blocked", "computed"):
            require(type(reads) is list and len(reads) == 2, "two blocked kernel read observations are required")
            expected = {sample["checkpoint"]["reader_a"], sample["checkpoint"]["reader_b"]}
            require(all(type(row) is dict and set(row) == {"tid", "fd", "syscall", "observed_ns"}
                        and integer(row["tid"], 1) and row["tid"] != pid and row["syscall"] == 0 and type(row["syscall"]) is int
                        and integer(row["observed_ns"], 1)
                        and integer(row["fd"], 3) for row in reads), "invalid kernel syscall observation")
            require({row["fd"] for row in reads} == expected and len({row["tid"] for row in reads}) == 2, "kernel readers differ from submitted owners")
        else:
            require(reads == [], "unexpected kernel observations outside witness")
    start, setup, blocked, computed, drained, soak, closed = stages
    for sample in (blocked, computed):
        for row in sample["blocked_reads"]:
            reader = "a" if row["fd"] == drained["checkpoint"]["reader_a"] else "b"
            require(drained["checkpoint"][f"read_{reader}_start_ns"] <= row["observed_ns"]
                    <= drained["checkpoint"][f"read_{reader}_end_ns"], "kernel observation lies outside joined I/O interval")
            require(row["observed_ns"] <= drained["checkpoint"]["compute_start_ns"] if sample is blocked
                    else row["observed_ns"] >= drained["checkpoint"]["compute_end_ns"], "kernel observations do not bracket CPU work")
    require(closed["checkpoint"]["managed_bytes"] == start["checkpoint"]["managed_bytes"], "managed allocations did not return to baseline")
    require(soak["checkpoint"]["managed_bytes"] == drained["checkpoint"]["managed_bytes"], "managed allocations grew during 10000-round soak")
    require(closed["fd_count"] == start["fd_count"], "descriptor cleanup did not return to baseline")
    require(blocked["fd_count"] == computed["fd_count"] == setup["fd_count"] + 4, "pipe descriptor ownership differs")
    require(drained["fd_count"] == soak["fd_count"] == setup["fd_count"], "descriptors leaked during drain/soak")
    require(max(sample["hwm_bytes"] for sample in stages) - start["rss_bytes"] <= limit, "process RSS high-water growth exceeded budget")
    witness_fields = ("compute_start_ns", "compute_end_ns", "checksum", "read_a_start_ns", "read_a_end_ns", "read_b_start_ns", "read_b_end_ns", "reader_a", "reader_b")
    require(all(soak["checkpoint"][key] == drained["checkpoint"][key] == closed["checkpoint"][key]
                for key in witness_fields), "witness changed after joined completion")
    require(all(computed["checkpoint"][key] == drained["checkpoint"][key] for key in ("compute_start_ns", "compute_end_ns", "checksum")), "computed interval changed after acknowledgement")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    args = parser.parse_args()
    require(args.evidence.is_file() and args.evidence.stat().st_size <= MAX_JSON_BYTES, "invalid evidence file")
    validate_report(decode(args.evidence.read_text()))
    print("PASS: kernel-pipe I/O/compute overlap, exactly-once delivery and bounded resources")


if __name__ == "__main__":
    main()
