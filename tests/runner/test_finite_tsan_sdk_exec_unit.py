#!/usr/bin/env python3
"""Sealed-byte and real mount-boundary tests; these are not native TSan proof.

No compiler is built or impersonated. Namespace tests run Python or /bin/sh only
as filesystem/process probes, and skip only when Bubblewrap's kernel namespace
prerequisite is unavailable. Production execution never has that fallback.
"""
import errno
import fcntl
import hashlib
import json
import mmap
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import finite_tsan_sdk as sdk
import finite_tsan_sdk_exec as sdk_exec

BWRAP_AVAILABLE = os.path.isfile(sdk_exec.BWRAP) and os.access(sdk_exec.BWRAP, os.X_OK)
CACHE = ROOT / '.seen/finite-tsan-unit-tests'
CACHE.mkdir(parents=True, exist_ok=True)


def row(payload):
    return {'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}


class SealedRuntimeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=CACHE)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / 'runtime.a'
        self.payload = b'!<arch>\n' + bytes(range(256)) * 300
        self.source.write_bytes(self.payload)

    def snapshot(self, expected=None):
        fd = sdk_exec.sealed_runtime(self.source, row(self.payload) if expected is None else expected)
        self.addCleanup(os.close, fd)
        return fd

    def test_snapshot_has_all_seals_and_only_read_access(self):
        fd = self.snapshot()
        seals = (fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW
                 | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL)
        self.assertEqual(fcntl.fcntl(fd, fcntl.F_GET_SEALS), seals)
        self.assertEqual(fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE, os.O_RDONLY)
        self.assertFalse(os.get_inheritable(fd))
        self.assertEqual(os.read(fd, len(self.payload) + 1), self.payload)
        with self.assertRaises(OSError):
            os.write(fd, b'changed')
        # Reopening the inode read/write does not bypass any of the seals.
        writable = os.open(f'/proc/self/fd/{fd}', os.O_RDWR | os.O_CLOEXEC)
        self.addCleanup(os.close, writable)
        for mutate in (lambda: os.pwrite(writable, b'X', 0),
                       lambda: os.ftruncate(writable, 0),
                       lambda: os.ftruncate(writable, len(self.payload) + 1),
                       lambda: mmap.mmap(writable, 0, access=mmap.ACCESS_WRITE),
                       lambda: fcntl.fcntl(writable, fcntl.F_ADD_SEALS, seals)):
            with self.subTest(mutate=mutate), self.assertRaises(OSError) as error:
                mutate()
            self.assertEqual(error.exception.errno, errno.EPERM)
        self.assertEqual(os.pread(fd, len(self.payload), 0), self.payload)

    def test_source_mutation_and_path_replacement_cannot_change_snapshot(self):
        fd = self.snapshot()
        self.source.write_bytes(b'X' * len(self.payload))
        self.source.rename(self.root / 'replaced-runtime')
        self.source.write_bytes(b'Y' * len(self.payload))
        self.assertEqual(os.read(fd, len(self.payload) + 1), self.payload)

    def test_complete_streaming_handles_short_reads_and_short_writes(self):
        real_read, real_write = os.read, os.write
        with mock.patch.object(sdk_exec.os, 'read',
                               side_effect=lambda fd, count: real_read(fd, min(count, 513))), \
                mock.patch.object(sdk_exec.os, 'write',
                                  side_effect=lambda fd, data: real_write(fd, data[:17])):
            fd = self.snapshot()
        self.assertEqual(os.read(fd, len(self.payload) + 1), self.payload)

    def test_same_size_wrong_digest_is_rejected_and_snapshot_closed(self):
        created = []
        create = os.memfd_create

        def record(*args):
            fd = create(*args)
            created.append(fd)
            return fd

        expected = row(self.payload)
        expected['sha256'] = '0' * 64
        with mock.patch.object(sdk_exec.os, 'memfd_create', side_effect=record):
            with self.assertRaisesRegex(ValueError, 'SHA-256 differs'):
                sdk_exec.sealed_runtime(self.source, expected)
        for fd in created:
            with self.assertRaises(OSError) as error:
                os.fstat(fd)
            self.assertEqual(error.exception.errno, errno.EBADF)

    def test_missing_empty_malformed_and_oversize_rows_are_rejected(self):
        for expected in (None, {}, {'bytes': 1}, {'bytes': 0, 'sha256': '0' * 64},
                         {'bytes': True, 'sha256': '0' * 64},
                         {'bytes': sdk_exec.MAX_RUNTIME_BYTES + 1, 'sha256': '0' * 64},
                         {'bytes': len(self.payload), 'sha256': ''},
                         {'bytes': len(self.payload), 'sha256': None}):
            with self.subTest(expected=expected), self.assertRaises(ValueError):
                sdk_exec.sealed_runtime(self.source, expected)

    def test_incorrect_size_is_rejected(self):
        for size in (len(self.payload) - 1, len(self.payload) + 1):
            with self.subTest(size=size), self.assertRaisesRegex(ValueError, 'byte count differs'):
                sdk_exec.sealed_runtime(self.source, {**row(self.payload), 'bytes': size})

    def test_truncation_after_initial_stat_is_rejected(self):
        real_read = os.read
        first = True

        def truncate_before_read(fd, count):
            nonlocal first
            if first:
                first = False
                self.source.write_bytes(self.payload[:13])
            return real_read(fd, count)

        with mock.patch.object(sdk_exec.os, 'read', side_effect=truncate_before_read):
            with self.assertRaisesRegex(ValueError, 'byte count differs while reading'):
                sdk_exec.sealed_runtime(self.source, row(self.payload))

    def test_growth_after_initial_stat_is_rejected(self):
        real_read = os.read
        first = True

        def grow_before_read(fd, count):
            nonlocal first
            if first:
                first = False
                with self.source.open('ab') as stream:
                    stream.write(b'unpinned suffix')
            return real_read(fd, count)

        with mock.patch.object(sdk_exec.os, 'read', side_effect=grow_before_read):
            with self.assertRaisesRegex(ValueError, 'byte count differs while reading'):
                sdk_exec.sealed_runtime(self.source, row(self.payload))

    def test_rejects_file_and_parent_symlinks_and_nonregular_files(self):
        link = self.root / 'link'
        link.symlink_to(self.source)
        parent = self.root / 'parent'
        parent.symlink_to(self.root, target_is_directory=True)
        fifo = self.root / 'fifo'
        os.mkfifo(fifo)
        for path in (link, parent / self.source.name, fifo, self.root):
            with self.subTest(path=path), self.assertRaises((OSError, ValueError)):
                sdk_exec.sealed_runtime(path, row(self.payload))


class ManifestAndArgumentTests(unittest.TestCase):
    def test_manifest_requires_explicit_digest_before_tool_discovery(self):
        for digest in (None, '', 'bad', 'A' * 64):
            with self.subTest(digest=digest), self.assertRaisesRegex(ValueError, 'explicit SHA-256'):
                sdk_exec.execute('/missing/manifest', digest, '/missing/clang', [])

    @unittest.skipUnless(BWRAP_AVAILABLE, 'Bubblewrap is not installed')
    def test_stale_manifest_digest_rejected_without_compiler_discovery(self):
        with tempfile.TemporaryDirectory(dir=CACHE) as directory:
            manifest = Path(directory).resolve() / 'manifest.json'
            manifest.write_text('{}')
            stale = hashlib.sha256(manifest.read_bytes()).hexdigest()
            manifest.write_text('{"changed":true}')
            with self.assertRaisesRegex(ValueError, 'after wrapper binding'):
                sdk_exec.execute(str(manifest), stale, '/missing/clang', [])

    def test_enforced_option_is_after_options_and_before_first_terminator(self):
        arguments = ['-c', 'input.c', '-o', 'object.o', '--', '-resource-dir', '@literal', '--']
        result = sdk_exec.resource_arguments(arguments)
        self.assertEqual(result, ['-c', 'input.c', '-o', 'object.o', '--no-default-config',
                                  '-resource-dir', sdk_exec.RESOURCE,
                                  '--', '-resource-dir', '@literal', '--'])
        self.assertEqual(arguments, ['-c', 'input.c', '-o', 'object.o',
                                     '--', '-resource-dir', '@literal', '--'])

    def test_without_terminator_enforced_option_is_last(self):
        result = sdk_exec.resource_arguments(['-c', 'input.c'])
        self.assertEqual(result[-2:], ['-resource-dir', sdk_exec.RESOURCE])

    def test_direct_resource_overrides_are_rejected_only_before_terminator(self):
        for option in ('-resource-dir', '-resource-dir=/bad', '--resource-dir',
                       '--resource-dir=/bad', '-Xclang=-resource-dir=/bad'):
            with self.subTest(option=option):
                with self.assertRaisesRegex(ValueError, 'explicit SDK resource override'):
                    sdk_exec.resource_arguments([option])
                self.assertEqual(sdk_exec.resource_arguments(['--', option])[-2:], ['--', option])

    def test_response_files_cannot_hide_overrides_or_terminators(self):
        with self.assertRaisesRegex(ValueError, 'response files'):
            sdk_exec.resource_arguments(['@mutable.rsp', '-c'])
        self.assertEqual(sdk_exec.resource_arguments(['--', '@filename'])[-2:],
                         ['--', '@filename'])

    def test_opaque_configs_and_environment_rewrites_fail_closed(self):
        for option in ('--config', '--config=mutable.cfg'):
            with self.subTest(option=option), self.assertRaisesRegex(ValueError, 'compiler configs'):
                sdk_exec.resource_arguments([option])
        with mock.patch.dict(os.environ, {'CCC_OVERRIDE_OPTIONS': '+-resource-dir=/bad'}):
            with self.assertRaisesRegex(ValueError, 'CCC_OVERRIDE_OPTIONS'):
                sdk_exec.execute('/missing/manifest', '0' * 64, '/missing/clang', [])


@unittest.skipUnless(BWRAP_AVAILABLE, 'Bubblewrap is not installed')
class NamespaceBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='finite-sdk-exec-', dir=CACHE)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.stock = self.root / 'stock'
        self.cache = self.root / 'cache'
        for root in (self.stock, self.cache):
            (root / 'lib/linux').mkdir(parents=True)
        (self.stock / 'include').mkdir()
        (self.stock / 'include/stock.h').write_text('genuine stock sentinel')
        (self.stock / 'stock-link').symlink_to('include/stock.h')
        self.payloads, self.runtime_fds = {}, {}
        for index, name in enumerate(sdk.RUNTIMES):
            (self.stock / 'lib/linux' / name).write_bytes(b'stock runtime')
            payload = ('pinned runtime ' + str(index)).encode()
            cached = self.cache / 'lib/linux' / name
            cached.write_bytes(payload)
            self.payloads[name] = payload.decode()
            fd = sdk_exec.sealed_runtime(cached, row(payload))
            self.addCleanup(os.close, fd)
            self.runtime_fds[name] = fd
        self.marker = self.root / 'private-tmp-marker'
        self.marker.write_text('already-private tmp survives')

    def run_namespace(self, command, **kwargs):
        previous = os.open('.', os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        try:
            os.chdir(self.root)
            arguments = sdk_exec.namespace_arguments(self.stock, self.runtime_fds, command,
                                                     kwargs.pop('driver_argv0', None))
        finally:
            os.fchdir(previous)
            os.close(previous)
        result = subprocess.run([sdk_exec.BWRAP, *arguments],
                                pass_fds=tuple(self.runtime_fds.values()), capture_output=True,
                                cwd=self.root, **kwargs)
        unavailable = ('No permissions to create new namespace', 'Creating new namespace failed',
                       'Failed to make / slave', 'setting up uid map',
                       'No permissions to creating new namespace')
        stderr = result.stderr.decode(errors='replace')
        if result.returncode and any(message in stderr for message in unavailable):
            self.skipTest('kernel does not permit the required Bubblewrap namespace: ' + stderr.strip())
        return result

    def test_real_mounts_use_stock_plus_pinned_bytes_after_cache_replacement(self):
        for name in sdk.RUNTIMES:
            source = self.cache / 'lib/linux' / name
            source.rename(source.with_name(name + '.old'))
            source.write_bytes(b'concurrently replaced cache bytes')
        probe = '''
import errno, json, os
from pathlib import Path
resource = Path('/seen-finite-tsan-sdk')
names = json.loads(os.environ['RUNTIME_NAMES'])
values = {name: (resource / 'lib/linux' / name).read_text() for name in names}
readonly = []
for path in [resource / 'include/stock.h', *[resource / 'lib/linux' / name for name in names]]:
    try:
        path.write_text('tamper')
    except OSError as error:
        readonly.append(error.errno == errno.EROFS)
closed = []
for fd in json.loads(os.environ['RUNTIME_FDS']):
    try:
        os.fstat(fd)
        closed.append(False)
    except OSError as error:
        closed.append(error.errno == errno.EBADF)
print(json.dumps({'values': values, 'stock': (resource / 'stock-link').read_text(),
                  'readonly': readonly, 'closed': closed, 'cwd': os.getcwd(),
                  'marker': Path(os.environ['TMP_MARKER']).read_text(),
                  'env': os.environ['SDK_TEST_ENV'], 'input': input(),
                  'pidns': os.readlink('/proc/self/ns/pid'),
                  'cgroup': Path('/proc/self/cgroup').read_text()}))
'''
        environment = {**os.environ, 'RUNTIME_NAMES': json.dumps(list(sdk.RUNTIMES)),
                       'RUNTIME_FDS': json.dumps(list(self.runtime_fds.values())),
                       'TMP_MARKER': str(self.marker), 'SDK_TEST_ENV': 'unchanged'}
        result = self.run_namespace([sys.executable, '-c', probe], env=environment, input=b'stdin kept\n')
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
        observed = json.loads(result.stdout)
        self.assertEqual(observed['values'], self.payloads)
        self.assertEqual(observed['stock'], 'genuine stock sentinel')
        self.assertTrue(all(observed['readonly']))
        self.assertTrue(all(observed['closed']))
        self.assertEqual(observed['cwd'], str(self.root))
        self.assertEqual(observed['marker'], 'already-private tmp survives')
        self.assertEqual(observed['env'], 'unchanged')
        self.assertEqual(observed['input'], 'stdin kept')
        self.assertEqual(observed['pidns'], os.readlink('/proc/self/ns/pid'))
        self.assertEqual(observed['cgroup'], Path('/proc/self/cgroup').read_text())

    def test_nonzero_exit_and_signal_status_are_not_hidden(self):
        result = self.run_namespace([sys.executable, '-c', 'raise SystemExit(23)'])
        self.assertEqual(result.returncode, 23, result.stderr.decode(errors='replace'))
        result = self.run_namespace([sys.executable, '-c',
                                     'import os, signal; os.kill(os.getpid(), signal.SIGTERM)'])
        self.assertEqual(result.returncode, 128 + signal.SIGTERM,
                         result.stderr.decode(errors='replace'))

    def test_argument_construction_inside_rootless_private_tmp_namespace(self):
        probe = '''
import json, os, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import finite_tsan_sdk_exec as launch
import finite_tsan_sdk as sdk
cache, stock = map(Path, sys.argv[2:4])
expected = json.loads(sys.argv[4])
fds = {}
for name in sdk.RUNTIMES:
    fds[name] = launch.sealed_runtime(cache / 'lib/linux' / name, expected[name])
    os.set_inheritable(fds[name], True)
command = [sys.executable, '-c',
    "from pathlib import Path;import json;"
    "r=Path('/seen-finite-tsan-sdk/lib/linux');"
    "print(json.dumps({'runtime':(r/'libclang_rt.tsan-x86_64.a').read_text(),"
    "'private_tmp':Path('/tmp/private-tmp-marker').read_text()}))"]
arguments = launch.namespace_arguments(stock, fds, command)
os.execv(launch.BWRAP, [launch.BWRAP, *arguments])
'''
        expected = {name: row(payload.encode()) for name, payload in self.payloads.items()}
        result = subprocess.run([
            sdk_exec.BWRAP, '--die-with-parent', '--bind', '/', '/',
            '--bind', str(self.root), '/tmp', '--', sys.executable, '-c', probe,
            str(ROOT / 'scripts'), str(self.cache), str(self.stock), json.dumps(expected),
        ], capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
        observed = json.loads(result.stdout)
        self.assertEqual(observed['runtime'], self.payloads['libclang_rt.tsan-x86_64.a'])
        self.assertEqual(observed['private_tmp'], self.marker.read_text())


if __name__ == '__main__':
    unittest.main()
