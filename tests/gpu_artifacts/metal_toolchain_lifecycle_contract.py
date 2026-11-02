"""POSIX process ownership only, not Apple SDK or Darwin accounting evidence.

Admission/accounting are isolated here; the child is a real detached Python
process, produces no SDK output, and receives real signals and resource limits.
"""
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import check_gpu_metal_toolchain as toolchain


class MetalScratchContract(unittest.TestCase):
    def test_deleted_file_does_not_skip_surviving_byte_ceiling(self):
        with patch.object(toolchain.os, 'walk', return_value=[('/unused', [], ['gone', 'survivor'])]), \
                patch.object(Path, 'lstat', side_effect=[
                    FileNotFoundError(), SimpleNamespace(st_size=128 * toolchain.MIB + 1)]):
            with self.assertRaises(toolchain.ToolchainError):
                toolchain._scratch_size('/unused')

    def test_other_stat_failures_and_ceilings_remain_errors(self):
        with patch.object(toolchain.os, 'walk', return_value=[('/unused', [], ['file'])]):
            with patch.object(Path, 'lstat', side_effect=PermissionError):
                with self.assertRaises(PermissionError):
                    toolchain._scratch_size('/unused')
            with patch.object(Path, 'lstat', return_value=SimpleNamespace(st_size=128 * toolchain.MIB + 1)):
                with self.assertRaises(toolchain.ToolchainError):
                    toolchain._scratch_size('/unused')
        with patch.object(toolchain.os, 'walk', return_value=[('/unused', [], ['gone'] * 1025)]):
            with self.assertRaises(toolchain.ToolchainError):
                toolchain._scratch_size('/unused')


@unittest.skipUnless(hasattr(os, 'fork') and hasattr(signal, 'pthread_sigmask'), 'requires POSIX signals')
class MetalTerminationContract(unittest.TestCase):
    def test_sigterm_during_acquisition_and_watchdog_reaps_detached_child(self):
        artifact_root = ROOT / '.seen/tmp'
        artifact_root.mkdir(parents=True, exist_ok=True)
        for during_acquisition, custom_handler in ((True, False), (False, False), (False, True)):
            with self.subTest(acquisition=during_acquisition, custom=custom_handler), \
                    tempfile.TemporaryDirectory(dir=artifact_root) as root:
                marker = Path(root) / 'child.json'
                read_fd, write_fd = os.pipe()
                launcher = os.fork()
                if launcher == 0:
                    os.close(read_fd)
                    def previous_handler(signum, frame):
                        if (signal.getsignal(signal.SIGTERM) is not previous_handler or
                                signal.pthread_sigmask(signal.SIG_BLOCK, set()) != set()):
                            os._exit(3)
                        os.write(write_fd, b'restored\n')
                        raise SystemExit(73)

                    signal.signal(signal.SIGTERM, previous_handler if custom_handler else signal.SIG_DFL)
                    signal.pthread_sigmask(signal.SIG_SETMASK, set())
                    real_popen = subprocess.Popen
                    owned = None

                    def spawn(*args, **kwargs):
                        nonlocal owned
                        owned = real_popen(*args, **kwargs)
                        deadline = time.monotonic() + 4
                        while not marker.exists():
                            if owned.poll() is not None or time.monotonic() >= deadline:
                                raise AssertionError('real child did not become ready')
                            time.sleep(0.01)
                        if during_acquisition:
                            os.write(write_fd, b'ready\n')
                            os.kill(os.getpid(), signal.SIGTERM)
                        return owned

                    def group_memory(*args):
                        os.write(write_fd, b'ready\n')
                        return 0

                    def installation(profile):
                        if owned is not None:
                            if owned.returncode != -signal.SIGKILL:
                                raise AssertionError('owned child was not killed and waited')
                            try:
                                os.waitpid(owned.pid, os.WNOHANG)
                            except ChildProcessError:
                                os.write(write_fd, b'reaped\n')
                            else:
                                raise AssertionError('owned child was not reaped')
                        return {}, {}

                    helper = ('import json,os,signal,time; '
                              'blocked=signal.pthread_sigmask(signal.SIG_BLOCK, set()); '
                              'signal.signal(signal.SIGTERM, signal.SIG_IGN); '
                              f'open({str(marker) + ".tmp"!r}, "w").write(json.dumps([os.getpid(),os.getpgrp(),list(blocked)])); '
                              f'os.replace({str(marker) + ".tmp"!r}, {str(marker)!r}); '
                              'time.sleep(60)')
                    try:
                        with patch.multiple(toolchain, _validate=lambda profile: None,
                                _installation=installation, _host=lambda profile: None,
                                _native=lambda: (None, None), _memory_budget=lambda system: 256 * toolchain.MIB,
                                _task=lambda *args: SimpleNamespace(virtual=1 << 48),
                                _group_memory=group_memory), patch.object(toolchain.subprocess, 'Popen', spawn):
                            toolchain.invoke({'sdk': {'path': root}, 'tools': {'metal': {'path': sys.executable}}},
                                             'metal', ['-c', helper])
                    except SystemExit as error:
                        os._exit(error.code)
                    except BaseException:
                        os._exit(1)
                    os._exit(2)

                os.close(write_fd)
                reaped_launcher = False
                try:
                    self.assertTrue(select.select([read_fd], [], [], 6)[0], 'launcher never became ready')
                    self.assertEqual(os.read(read_fd, 6), b'ready\n')
                    if not during_acquisition:
                        os.kill(launcher, signal.SIGTERM)
                    deadline = time.monotonic() + 6
                    while True:
                        waited, status = os.waitpid(launcher, os.WNOHANG)
                        if waited:
                            reaped_launcher = True
                            break
                        self.assertLess(time.monotonic(), deadline, 'launcher failed to terminate')
                        time.sleep(0.01)
                    if custom_handler:
                        self.assertTrue(os.WIFEXITED(status), f'launcher wait status: {status}')
                        self.assertEqual(os.WEXITSTATUS(status), 73)
                    else:
                        self.assertTrue(os.WIFSIGNALED(status), f'launcher wait status: {status}')
                        self.assertEqual(os.WTERMSIG(status), signal.SIGTERM)
                    report = os.read(read_fd, 4096)
                    self.assertIn(b'reaped\n', report)
                    if custom_handler:
                        self.assertIn(b'restored\n', report)
                    child, group, blocked = json.loads(marker.read_text())
                    self.assertEqual(child, group)
                    self.assertNotIn(signal.SIGTERM, blocked)
                    self.assertNotIn(signal.SIGINT, blocked)
                    with self.assertRaises(ProcessLookupError):
                        os.kill(child, 0)
                finally:
                    os.close(read_fd)
                    if not reaped_launcher:
                        os.kill(launcher, signal.SIGKILL)
                        os.waitpid(launcher, 0)
                    if marker.exists():
                        try:
                            os.killpg(json.loads(marker.read_text())[1], signal.SIGKILL)
                        except ProcessLookupError:
                            pass


if __name__ == '__main__':
    unittest.main()
