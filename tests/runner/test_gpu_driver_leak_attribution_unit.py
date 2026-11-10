#!/usr/bin/env python3
"""Scoped GPU driver leak attribution must never admit Seen or unbounded leaks."""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import gpu_driver_leak_attribution as attribution


def report(*blocks):
    lines = ["==1==ERROR: LeakSanitizer: detected memory leaks", ""]
    for kind, size, objects, caller in blocks:
        lines += [f"{kind} leak of {size} byte(s) in {objects} object(s) allocated from:",
                  "    #0 0x55d0aa216a6e  (/work/fixture+0x216a6e) (BuildId: 00)",
                  f"    #1 0x7b358dbe7b8d  ({caller}) (BuildId: 01)",
                  "    #2 0x55d0aa001234  (/work/fixture+0x1234) (BuildId: 00)", ""]
    total = sum(size for _, size, _, _ in blocks), sum(objects for _, _, objects, _ in blocks)
    lines.append(f"SUMMARY: AddressSanitizer: {total[0]} byte(s) leaked in {total[1]} allocation(s).")
    return "\n".join(lines) + "\n"


CUDA = report(("Direct", 56, 1, "/usr/lib/libcuda.so.1+0x30eb1a"),
              ("Indirect", 127, 3, "/usr/lib/libcuda.so.1+0x48dd10"))
VULKAN = report(("Direct", 184, 1, "/usr/lib/libdbus-1.so.3+0x2d02f"),
                ("Direct", 56, 1, "<unknown module>"))


class AttributionTests(unittest.TestCase):
    def test_clean_fixture_is_accepted_and_driver_leaks_within_control_are_accepted(self):
        self.assertEqual(attribution.attribute("cuda", "clean\n", CUDA)["fixture"], (0, 0))
        self.assertEqual(attribution.attribute("cuda", CUDA, CUDA)["fixture"], (183, 4))
        self.assertEqual(attribution.attribute("vulkan", VULKAN, VULKAN)["control"], (240, 2))

    def test_seen_or_libc_allocation_is_never_attributed(self):
        for caller in ("/work/fixture+0x99", "/usr/lib/libc.so.6+0x10", "/usr/lib/libseen_runtime.so+0x1"):
            with self.subTest(caller=caller):
                fixture = report(("Direct", 37, 1, caller))
                with self.assertRaises(ValueError):
                    attribution.attribute("cuda", fixture, CUDA)

    def test_backends_do_not_share_driver_modules(self):
        with self.assertRaises(ValueError):
            attribution.attribute("vulkan", CUDA, VULKAN)
        with self.assertRaises(ValueError):
            attribution.attribute("cuda", VULKAN, CUDA)

    def test_fixture_cannot_exceed_same_run_control(self):
        larger_bytes = report(("Direct", 184, 4, "/usr/lib/libcuda.so.1+0x1"))
        more_objects = report(("Direct", 100, 5, "/usr/lib/libcuda.so.1+0x1"))
        for fixture in (larger_bytes, more_objects):
            with self.subTest(fixture=fixture.splitlines()[2]):
                with self.assertRaises(ValueError):
                    attribution.attribute("cuda", fixture, CUDA)
        with self.assertRaises(ValueError):
            attribution.attribute("cuda", CUDA, "clean\n")

    def test_control_leaking_outside_driver_is_rejected(self):
        with self.assertRaises(ValueError):
            attribution.attribute("cuda", "clean\n", report(("Direct", 37, 1, "/work/control+0x1")))

    def test_non_leak_errors_and_inconsistent_reports_are_rejected(self):
        bad = ("==1==ERROR: AddressSanitizer: heap-use-after-free on address 0x1\n",
               CUDA.replace("183 byte(s)", "180 byte(s)"),
               CUDA.replace("    #1 ", "    #5 "),
               "\n".join(CUDA.splitlines()[:-1]) + "\n",
               "==1==ERROR: LeakSanitizer: detected memory leaks\n")
        for fixture in bad:
            with self.subTest(fixture=fixture[:60]):
                with self.assertRaises(ValueError):
                    attribution.attribute("cuda", fixture, CUDA)


if __name__ == "__main__":
    unittest.main()
