#!/usr/bin/env python3
"""Adversarial data-only tests; this script never launches a Seen executable."""
import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from io_overlap_evidence import COUNTS, FIELDS, STAGES, compute_checksum, decode, validate_report


def fixture():
    pid = 123
    stages = []
    for index, stage in enumerate(STAGES):
        event = {key: 0 for key in FIELDS}
        event.update(schema="seen-io-checkpoint-v1", stage=stage, pid=pid, saturated=False,
                     managed_bytes=100 if stage in ("start", "closed") else 1000,
                     reader_a=5 if index >= 2 else -1, reader_b=7 if index >= 2 else -1)
        count = 0 if index < 2 else 2 if index < 5 else 20002
        finished = count if index >= 4 else 0
        event.update(zip(COUNTS, (count, count, finished, finished, finished * 16)))
        if count:
            event.update(peak_active=2, peak_submitted=2, peak_owned_bytes=128, peak_metadata_bytes=256)
        if index >= 3:
            event.update(compute_start_ns=300, compute_end_ns=500, checksum=compute_checksum(pid))
        if index >= 4:
            event.update(read_a_start_ns=100, read_b_start_ns=110, read_a_end_ns=700, read_b_end_ns=710)
        reads = []
        if stage in ("blocked", "computed"):
            reads = [{"tid": pid + i, "fd": fd, "syscall": 0, "observed_ns": 200 if stage == "blocked" else 600}
                     for i, fd in enumerate((5, 7), start=1)]
        stages.append({"checkpoint": event, "rss_bytes": 10000, "hwm_bytes": 12000,
                       "fd_count": 7 if stage in ("blocked", "computed") else 3, "blocked_reads": reads})
    return {"schema": "seen-io-overlap-v1", "target": "linux-x86_64", "backend": "explicit-worker-kernel-pipes",
            "profile": "fast", "classification": "verified", "storage_overlap_verified": False,
            "source_commit": "a" * 40, "source_sha256": "b" * 64, "executable_sha256": "c" * 64,
            "compiler_sha256": "d" * 64, "command": ["/fixture"],
            "machine": dict(system="Linux", release="6.1", machine="x86_64", cpu="test CPU"),
            "cgroup": dict(path="/fixture.scope", memory_max_bytes=1 << 30, swap_max_bytes=0, pids_max=24, oom_group=1),
            "rss_growth_limit_bytes": 1 << 20, "stages": stages}


class EvidenceContract(unittest.TestCase):
    def test_complete_report(self):
        for profile in ("fast", "release", "undefined", "address"):
            report = fixture(); report["profile"] = profile
            self.assertIs(validate_report(report), report)

    def test_invalid_reports(self):
        mutations = [
            lambda r: r.update(storage_overlap_verified=True),
            lambda r: r.update(classification="compile-only"),
            lambda r: r.update(target="linux-arm64"),
            lambda r: r.update(compiler_sha256="missing"),
            lambda r: r.update(command=[]),
            lambda r: r.update(rss_growth_limit_bytes=1 << 30),
            lambda r: r["cgroup"].update(swap_max_bytes=1),
            lambda r: r["cgroup"].update(pids_max=25),
            lambda r: r["cgroup"].update(memory_max_bytes=True),
            lambda r: r["cgroup"].update(oom_group=0),
            lambda r: r["stages"].pop(),
            lambda r: r["stages"][2].update(blocked_reads=[]),
            lambda r: r["stages"][3]["blocked_reads"][1].update(fd=5),
            lambda r: r["stages"][2]["blocked_reads"][1].update(tid=124),
            lambda r: r["stages"][2]["blocked_reads"][0].update(syscall=True),
            lambda r: r["stages"][2]["blocked_reads"][0].update(observed_ns=400),
            lambda r: r["stages"][3]["blocked_reads"][0].update(observed_ns=400),
            lambda r: r["stages"][4]["checkpoint"].update(read_a_end_ns=400),
            lambda r: r["stages"][3]["checkpoint"].update(checksum=0),
            lambda r: r["stages"][5]["checkpoint"].update(accepted=20001),
            lambda r: r["stages"][6]["checkpoint"].update(delivered=20001),
            lambda r: r["stages"][4]["checkpoint"].update(saturated=True),
            lambda r: r["stages"][2]["checkpoint"].update(peak_owned_bytes=129),
            lambda r: r["stages"][2]["checkpoint"].update(peak_metadata_bytes=8193),
            lambda r: r["stages"][5]["checkpoint"].update(managed_bytes=1001),
            lambda r: r["stages"][6]["checkpoint"].update(managed_bytes=101),
            lambda r: r["stages"][2].update(fd_count=8),
            lambda r: r["stages"][5].update(fd_count=4),
            lambda r: r["stages"][6].update(fd_count=4),
            lambda r: r["stages"][5].update(hwm_bytes=1 << 25),
            lambda r: r["stages"][6]["checkpoint"].update(compute_start_ns=301),
            lambda r: r["stages"][6]["checkpoint"].update(pid=False),
            lambda r: r["stages"][0]["checkpoint"].update(accepted=False),
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                report = copy.deepcopy(fixture()); mutation(report)
                with self.assertRaises(ValueError):
                    validate_report(report)

    def test_untrusted_json(self):
        for text in ('{"pid":1,"pid":2}', '{"x":NaN}', '{"x":Infinity}', ' ' * 1048577):
            with self.subTest(text=text[:40]), self.assertRaises(ValueError):
                decode(text)


if __name__ == "__main__":
    unittest.main()
