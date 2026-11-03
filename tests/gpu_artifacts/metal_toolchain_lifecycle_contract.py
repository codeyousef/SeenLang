"""POSIX process ownership only, not Apple SDK or Darwin accounting evidence.

Admission/accounting are isolated here; the child is a real detached Python
process, produces no SDK output, and receives real signals and resource limits.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import plistlib
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


class MetalToolBindingContract(unittest.TestCase):
    """Filesystem/exec identity tests, never Apple tool output or certification."""

    def setUp(self):
        artifact_root = ROOT / '.seen/tmp'
        artifact_root.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=artifact_root)
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()
        sdk = self.directory / 'SDK'
        sdk.mkdir()
        settings = plistlib.dumps({'Version': '26.5', 'CanonicalName': 'macosx26.5'})
        (sdk / 'SDKSettings.plist').write_bytes(settings)
        self.target = self.directory / 'air-lld'
        self.target.write_bytes(b'Binding data only; not an SDK executable.\n')
        self.target.chmod(0o700)
        self.profile = {
            'schema': toolchain.SCHEMA, 'platform': {'system': 'Darwin', 'machine': 'arm64'},
            'targets': list(toolchain.TARGETS), 'languages': dict(toolchain.LANGUAGES),
            'sdk': {'path': str(sdk), 'settings_sha256': hashlib.sha256(settings).hexdigest()},
            'tools': {},
        }
        self.bind(self.target)

    def bind(self, target):
        with target.open('rb') as source:
            digest = hashlib.file_digest(source, 'sha256').hexdigest()
        for name in ('metal', 'metallib'):
            invocation = self.directory / name
            invocation.unlink(missing_ok=True)
            invocation.symlink_to(target)
            self.profile['tools'][name] = {
                'path': str(invocation), 'resolved_path': str(target), 'sha256': digest,
            }
        self.profile['tools']['metal']['version_line'] = 'identity-test-only'

    def test_retargeted_alias_is_rejected_even_when_target_bytes_match(self):
        toolchain._validate(self.profile)
        toolchain._installation(self.profile)
        replacement = self.directory / 'different-target'
        replacement.write_bytes(self.target.read_bytes())
        replacement.chmod(0o700)
        alias = self.directory / 'metallib'
        alias.unlink()
        alias.symlink_to(replacement)
        with self.assertRaises(toolchain.ToolchainError):
            toolchain._installation(self.profile)

    def test_changed_pinned_binary_is_rejected(self):
        toolchain._installation(self.profile)
        self.target.write_bytes(b'Changed executable bytes.\n')
        with self.assertRaises(toolchain.ToolchainError):
            toolchain._installation(self.profile)

    def test_unresolved_invocation_parent_is_rejected(self):
        alias = self.directory / 'parent-alias'
        alias.symlink_to(self.directory, target_is_directory=True)
        self.profile['tools']['metallib']['path'] = str(alias / 'metallib')
        toolchain._validate(self.profile)
        with self.assertRaises(toolchain.ToolchainError):
            toolchain._installation(self.profile)

    def test_old_and_incomplete_policy_bindings_fail_closed(self):
        old = copy.deepcopy(self.profile)
        old['schema'] = 'seen-metal-official-toolchain-v1'
        for tool in old['tools'].values():
            del tool['resolved_path']
        missing = copy.deepcopy(self.profile)
        del missing['tools']['metallib']['resolved_path']
        relative = copy.deepcopy(self.profile)
        relative['tools']['metallib']['resolved_path'] = 'air-lld'
        for profile in (old, missing, relative):
            with self.subTest(profile=profile), self.assertRaises(toolchain.ToolchainError):
                toolchain._validate(profile)

    @unittest.skipUnless(sys.platform == 'darwin', 'requires real XNU map-limit semantics')
    def test_real_darwin_invocation_keeps_finite_limits_without_rejecting_current_mappings(self):
        self.bind(Path(sys.executable).resolve())
        self.profile['platform']['machine'] = toolchain.platform.machine()
        script = ("import json,resource; print(json.dumps(["
                  "resource.getrlimit(resource.RLIMIT_AS),"
                  "resource.getrlimit(resource.RLIMIT_DATA)]))")
        status, output, _ = toolchain.invoke(self.profile, 'metallib', ['-c', script])
        self.assertEqual(status, 0)
        for soft, hard in json.loads(output):
            self.assertGreater(soft, 0)
            self.assertLessEqual(soft, hard)
            self.assertLess(hard, (1 << 63) - 1)

    def posix_invoke(self, script):
        # Isolate Darwin accounting only; execution, argv[0], signals, limits
        # and pre/post filesystem binding checks are real POSIX operations.
        with patch.multiple(toolchain, _host=lambda profile: None,
                _native=lambda: (None, None), _memory_budget=lambda system: 256 * toolchain.MIB,
                _task=lambda *args: SimpleNamespace(virtual=1 << 48),
                _group_memory=lambda *args: 0):
            return toolchain.invoke(self.profile, 'metallib', ['-c', script])

    @unittest.skipUnless(sys.platform == 'linux', 'requires /proc exec identity')
    def test_resolved_executable_keeps_the_logical_invocation_name(self):
        executable = Path(sys.executable).resolve()
        self.bind(executable)
        script = ("import json,os; print(json.dumps([os.readlink('/proc/self/exe'),"
                  "open('/proc/self/cmdline','rb').read().split(b'\\0')[0].decode()]))")
        status, output, _ = self.posix_invoke(script)
        self.assertEqual(status, 0)
        self.assertEqual(json.loads(output), [str(executable), str(self.directory / 'metallib')])

    @unittest.skipUnless(sys.platform == 'linux', 'requires /proc exec identity')
    def test_alias_swap_cannot_redirect_exec_and_cannot_be_reported_successful(self):
        executable = Path(sys.executable).resolve()
        self.bind(executable)
        marker = self.directory / 'executed.json'
        script = ("import json,os; "
                  f"open({str(marker)!r},'w').write(json.dumps(os.readlink('/proc/self/exe')))")
        real_popen = subprocess.Popen
        def replace_alias(*args, **kwargs):
            alias = self.directory / 'metallib'
            alias.unlink()
            alias.symlink_to('/usr/bin/false')
            return real_popen(*args, **kwargs)
        with patch.object(toolchain.subprocess, 'Popen', replace_alias):
            with self.assertRaises(toolchain.ToolchainError):
                self.posix_invoke(script)
        self.assertEqual(json.loads(marker.read_text()), str(executable))


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
                            toolchain.invoke({'sdk': {'path': root}, 'tools': {'metal': {
                                'path': sys.executable, 'resolved_path': str(Path(sys.executable).resolve())}}},
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
