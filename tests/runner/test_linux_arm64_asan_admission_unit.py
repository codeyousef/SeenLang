#!/usr/bin/env python3
"""Control boundaries and owned-child lifetime; these tests never qualify ARM."""
import os
from pathlib import Path
import select
import signal
import struct
import sys
import tempfile
import subprocess
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import linux_arm64_asan_admission as admission

MARKER = (f'ARM_ASAN_CONTROL soft={admission.CAP} hard={admission.CAP} '
          f'scale=3 offset={admission.SHADOW_OFFSET} aslr_disabled=0\n').encode()
HEAP_REPORT = (b'ERROR: AddressSanitizer: heap-buffer-overflow on address 0x1234\n'
               b'WRITE of size 1 at 0x1234 thread T0\n'
               b'SUMMARY: AddressSanitizer: heap-buffer-overflow\n')
RESERVATION_REPORT = (b'AddressSanitizer: ReserveShadowMemoryRange failed while trying to map '
                      b"0x200001000 bytes. Perhaps you're using ulimit -v or ulimit -d\n")

# The 39-bit default example in pinned compiler-rt/asan/asan_mapping.h.
LAYOUT = b'''|| `[0x2000000000, 0x7fffffffff]` || HighMem ||
|| `[0x1400000000, 0x1fffffffff]` || HighShadow ||
|| `[0x1200000000, 0x13ffffffff]` || ShadowGap ||
|| `[0x1000000000, 0x11ffffffff]` || LowShadow ||
|| `[0x0000000000, 0x0fffffffff]` || LowMem ||
'''
MAPS = b'''1000000000-1200000000 rw-p 00000000 00:00 0
1200000000-1400000000 ---p 00000000 00:00 0
1400000000-2000000000 rw-p 00000000 00:00 0
'''


class ControlDecisionTests(unittest.TestCase):
    def test_clean_requires_completed_limit_mapping_and_no_finding(self):
        clean = MARKER + MAPS + b'ARM_ASAN_CONTROL clean-complete\n'
        admission.validate_control('clean', 0, clean, b'')
        for status, out, err in ((1, clean, b''), (0, b'clean-complete\n', b''),
                                 (0, MARKER, b''), (0, clean, HEAP_REPORT),
                                 (0, clean, b'SUMMARY: AddressSanitizer: heap-buffer-overflow'),
                                 (0, clean.replace(b'aslr_disabled=0', b'aslr_disabled=1'), b''),
                                 (0, clean.replace(str(admission.CAP).encode(), b'0'), b'')):
            with self.subTest(status=status, out=out, err=err), self.assertRaises(RuntimeError):
                admission.validate_control('clean', status, out, err)

    def test_heap_requires_complete_real_finding_not_just_failure(self):
        admission.validate_control('heap', -signal.SIGABRT, MARKER, HEAP_REPORT)
        for status, out, err in ((1, MARKER, HEAP_REPORT), (0, MARKER, HEAP_REPORT),
                                 (-signal.SIGKILL, MARKER, HEAP_REPORT),
                                 (-signal.SIGABRT, MARKER, b'failed'),
                                 (-signal.SIGABRT, MARKER, HEAP_REPORT.replace(b'WRITE of size 1', b'READ of size 1')),
                                 (-signal.SIGABRT, b'', HEAP_REPORT),
                                 (-signal.SIGABRT, MARKER, HEAP_REPORT.split(b'SUMMARY:')[0])):
            with self.subTest(status=status, out=out, err=err), self.assertRaises(RuntimeError):
                admission.validate_control('heap', status, out, err)

    def test_insufficient_limit_requires_shadow_reservation_failure_before_main(self):
        admission.validate_control('insufficient', -signal.SIGABRT, b'', RESERVATION_REPORT)
        for status, out, err in ((1, b'', RESERVATION_REPORT), (0, b'', RESERVATION_REPORT),
                                 (-signal.SIGABRT, MARKER, RESERVATION_REPORT),
                                 (-signal.SIGABRT, b'', HEAP_REPORT),
                                 (-signal.SIGABRT, b'', b'AddressSanitizer failed to allocate (error code: 12)'),
                                 (-signal.SIGKILL, b'', RESERVATION_REPORT)):
            with self.subTest(status=status, out=out, err=err), self.assertRaises(RuntimeError):
                admission.validate_control('insufficient', status, out, err)


class LayoutDecisionTests(unittest.TestCase):
    def test_layout_requires_both_runtime_boundaries_and_real_kernel_maps(self):
        evidence = admission.layout_evidence(MAPS, LAYOUT)
        self.assertEqual(evidence['scale'], 3)
        self.assertEqual(evidence['offset'], 64 * 1024**3)
        self.assertLess(evidence['mapped_bytes'], admission.CAP)
        for maps in (b'', MAPS.replace(b'---p', b'rw-p'),
                     MAPS.replace(b'1400000000-2000000000', b'1500000000-2000000000'),
                     MAPS.replace(b'1000000000-1200000000', b'1000000000-1100000000')):
            with self.subTest(maps=maps), self.assertRaises(RuntimeError):
                admission.layout_evidence(maps, LAYOUT)

    def test_layout_rejects_caller_style_metadata_duplicates_and_other_layouts(self):
        for layout in (b'{"layout": "arm64", "passed": true}',
                       LAYOUT + LAYOUT, LAYOUT.replace(b'0x1000000000', b'0x007fff8000'),
                       LAYOUT.replace(b'HighShadow', b'Unknown')):
            with self.subTest(layout=layout), self.assertRaises(RuntimeError):
                admission.layout_evidence(MAPS, layout)


class NativeBoundaryTests(unittest.TestCase):
    def setUp(self):
        cache = ROOT / '.seen/linux-arm64-asan-unit'
        cache.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_elf_requires_native_machine_class_byte_order_and_executable_type(self):
        header = bytearray(64)
        header[:6] = b'\x7fELF\x02\x01'
        struct.pack_into('<HH', header, 16, 3, 183)
        path = self.root / 'header-only-not-executed'
        path.write_bytes(header)
        self.assertEqual(admission.native_elf(path)['machine'], 183)
        variants = []
        for offset, value in ((4, 1), (5, 2), (18, 62), (16, 1)):
            invalid = header.copy()
            invalid[offset] = value
            variants.append(invalid)
        variants.append(header[:20])
        for invalid in variants:
            path.write_bytes(invalid)
            with self.subTest(header=invalid), self.assertRaises(RuntimeError):
                admission.native_elf(path)

    def test_evidence_read_rejects_links_and_oversized_files(self):
        path = self.root / 'record'
        path.write_bytes(b'bounded')
        link = self.root / 'link'
        link.symlink_to(path)
        with self.assertRaises(RuntimeError):
            admission.bounded_read(link)
        with self.assertRaises(RuntimeError):
            admission.bounded_read(path, maximum=2)

    def assert_process_exited(self, pid):
        try:
            descriptor = os.pidfd_open(pid)
        except ProcessLookupError:
            return
        try:
            ready = select.poll()
            ready.register(descriptor, select.POLLIN)
            self.assertTrue(ready.poll(5000), "owned child remained alive after command drain")
        finally:
            os.close(descriptor)

    def test_successful_leader_does_not_leave_owned_descendant_running(self):
        program = ("import os,time\n"
                   "pid=os.fork()\n"
                   "if pid == 0:\n"
                   "    time.sleep(60)\n"
                   "    os._exit(0)\n"
                   "print(pid,flush=True)\n"
                   "os._exit(0)\n")
        status, out, _ = admission.command(
            [sys.executable, "-c", program], self.root, "descendant", dict(os.environ), timeout=10)
        self.assertEqual(status, 0)
        self.assert_process_exited(int(out.strip()))

    def test_deadline_is_failure_and_owned_process_is_drained(self):
        program = "import os,time; print(os.getpid(),flush=True); time.sleep(60)"
        with self.assertRaises(subprocess.TimeoutExpired):
            admission.command([sys.executable, "-c", program], self.root, "deadline",
                              dict(os.environ), timeout=2)
        self.assert_process_exited(int((self.root / "deadline.out").read_bytes().strip()))

    def test_child_environment_removes_suppressions_preloads_and_build_overrides(self):
        names = ('LD_PRELOAD', 'LD_LIBRARY_PATH', 'ASAN_OPTIONS', 'LSAN_OPTIONS',
                 'ASAN_SYMBOLIZER_PATH', 'CFLAGS', 'CCC_OVERRIDE_OPTIONS')
        with mock.patch.dict(os.environ, dict.fromkeys(names, 'caller-controlled')):
            env = admission.environment()
        self.assertEqual(env['LSAN_OPTIONS'], 'exitcode=23')
        for name in names:
            self.assertNotEqual(env.get(name), 'caller-controlled')


if __name__ == '__main__':
    unittest.main()
