#!/usr/bin/env python3
"""Consume pinned TSan bytes through sealed files in a private resource mount.

The cache is only an input: neither its resource pathname nor a mutable runtime
file is passed to Clang. Bubblewrap reconstructs the current root (including any
already-private /tmp), then overlays four sealed snapshots on the stock resource
tree. This adds no PID namespace, resource limit, or independent admission scope.

Accepted calls preserve argv and inherited execution context. Pre-terminator
response files and explicit compiler configs fail closed: either can inject a
hidden terminator ahead of the enforced resource option. Bubblewrap propagates
ordinary exits and represents signal death as shell status 128+signal; it does
not preserve the raw waitpid signal/exit distinction.
"""
from contextlib import ExitStack
import fcntl
import hashlib
import os
from pathlib import Path
import re
import signal
import stat
import sys

import finite_tsan_sdk as sdk

RESOURCE = '/seen-finite-tsan-sdk'
BWRAP = '/usr/bin/bwrap'
MAX_RUNTIME_BYTES = 128 * 1024**2
CHUNK_BYTES = 64 * 1024


def require_sha256(value, label):
    sdk.require(isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None,
                f'{label} requires an explicit SHA-256 digest')


def open_regular(path):
    """Open every component without following links, retaining directory handles."""
    path = Path(path)
    sdk.require(path.is_absolute() and '..' not in path.parts, 'runtime path is not absolute')
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW
    with ExitStack() as directories:
        directory = os.open('/', flags | os.O_DIRECTORY)
        directories.callback(os.close, directory)
        for component in path.parts[1:-1]:
            directory = os.open(component, flags | os.O_DIRECTORY, dir_fd=directory)
            directories.callback(os.close, directory)
        fd = os.open(path.name, flags | os.O_NONBLOCK, dir_fd=directory)
    try:
        sdk.require(stat.S_ISREG(os.fstat(fd).st_mode), 'runtime input is not a regular file')
        return fd
    except BaseException:
        os.close(fd)
        raise


def sealed_runtime(path, expected):
    """Return a read-only, fully sealed descriptor of independently verified bytes."""
    sdk.require(isinstance(expected, dict) and set(expected) == {'bytes', 'sha256'},
                'runtime digest row is missing or malformed')
    size = expected['bytes']
    sdk.require(type(size) is int and 0 < size <= MAX_RUNTIME_BYTES,
                'runtime byte count is empty or exceeds ceiling')
    require_sha256(expected['sha256'], 'runtime')
    sdk.require(hasattr(os, 'memfd_create') and hasattr(os, 'MFD_ALLOW_SEALING')
                and all(hasattr(fcntl, name) for name in (
                    'F_ADD_SEALS', 'F_GET_SEALS', 'F_SEAL_WRITE', 'F_SEAL_SHRINK',
                    'F_SEAL_GROW', 'F_SEAL_SEAL')), 'sealed memfds are unavailable')
    seals = (fcntl.F_SEAL_WRITE | fcntl.F_SEAL_SHRINK
             | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SEAL)
    with ExitStack() as files:
        source = open_regular(path)
        files.callback(os.close, source)
        sdk.require(os.fstat(source).st_size == size, 'runtime byte count differs')
        snapshot = os.memfd_create('seen-finite-tsan-runtime',
                                   os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING)
        files.callback(os.close, snapshot)
        hashed = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(source, min(CHUNK_BYTES, size - total + 1))
            if not chunk:
                break
            total += len(chunk)
            sdk.require(total <= size, 'runtime byte count differs while reading')
            hashed.update(chunk)
            remaining = memoryview(chunk)
            while remaining:
                written = os.write(snapshot, remaining)
                sdk.require(written > 0, 'runtime snapshot write made no progress')
                remaining = remaining[written:]
        sdk.require(total == size and os.fstat(source).st_size == size,
                    'runtime byte count differs while reading')
        sdk.require(hashed.hexdigest() == expected['sha256'], 'runtime SHA-256 differs')
        fcntl.fcntl(snapshot, fcntl.F_ADD_SEALS, seals)
        sdk.require(fcntl.fcntl(snapshot, fcntl.F_GET_SEALS) == seals,
                    'runtime snapshot sealing failed')
        # Drop even the writable descriptor. Seals also forbid modifying the inode
        # through another /proc descriptor, truncation, or a shared writable mmap.
        return os.open(f'/proc/self/fd/{snapshot}', os.O_RDONLY | os.O_CLOEXEC)


def resource_arguments(arguments):
    """Place the enforced resource option before the caller's first terminator."""
    arguments = list(arguments)
    end = arguments.index('--') if '--' in arguments else len(arguments)
    for argument in arguments[:end]:
        sdk.require(not any(argument == option or argument.startswith(option + '=')
                            for option in ('-resource-dir', '--resource-dir')),
                    'explicit SDK resource override')
        # Response files can change after inspection and can inject a terminator.
        # Preserve argv rather than attempting an incomplete LLVM response parser.
        sdk.require(not argument.startswith('@'),
                    'response files cannot enforce the pinned SDK resource option')
        sdk.require(argument != '--config' and not argument.startswith('--config='),
                    'explicit compiler configs cannot enforce the pinned SDK resource option')
        sdk.require(not argument.startswith(('-Xclang=-resource-dir',
                                             '-Xclang=--resource-dir')),
                    'explicit SDK resource override')
    return (arguments[:end] + ['--no-default-config', '-resource-dir', RESOURCE]
            + arguments[end:])


def namespace_arguments(stock_resource, runtime_fds, command, driver_argv0=None):
    """Build a private root without placing a mount in a host-renamable cache."""
    root = os.stat('/', follow_symlinks=False)
    sdk.require(root.st_uid == 0 and root.st_mode & 0o022 == 0,
                'host root is not protected against untrusted renames')
    sdk.require(set(runtime_fds) == set(sdk.RUNTIMES), 'runtime snapshot set differs')
    arguments = ['--tmpfs', '/']
    with os.scandir('/') as entries:
        for entry in sorted(entries, key=lambda row: row.name):
            destination = '/' + entry.name
            if destination == RESOURCE:
                continue
            mode = entry.stat(follow_symlinks=False).st_mode
            if stat.S_ISLNK(mode):
                arguments += ['--symlink', os.readlink(destination), destination]
            else:
                # dev-bind, unlike bind, retains existing device access. Mounts
                # below /tmp, /dev, /proc, etc. are recursively retained by bwrap.
                arguments += ['--dev-bind', destination, destination]
    arguments += ['--ro-bind', str(stock_resource), RESOURCE]
    for name in sdk.RUNTIMES:
        arguments += ['--ro-bind-data', str(runtime_fds[name]),
                      RESOURCE + '/lib/linux/' + name]
    arguments += ['--remount-ro', '/', '--chdir', os.getcwd()]
    if driver_argv0 is not None:
        arguments += ['--argv0', driver_argv0]
    return arguments + ['--'] + list(command)


def execute(manifest_path, expected_sha256, compiler, arguments):
    require_sha256(expected_sha256, 'manifest')
    sdk.require(not os.environ.get('CCC_OVERRIDE_OPTIONS'),
                'CCC_OVERRIDE_OPTIONS can rewrite the pinned SDK resource option')
    clang_arguments = resource_arguments(arguments)
    driver_argv0 = os.environ.get('SEEN_TSAN_CLANG_DRIVER_ARGV0')
    if driver_argv0 is not None:
        driver = Path(driver_argv0)
        sdk.require(driver.parent == Path(compiler).parent
                    and driver.name in ('clang', 'clang++', 'cc', 'c++'),
                    'SDK compiler driver argv0 differs')
    # A caller-controlled PATH must not substitute a different mount helper.
    bwrap = BWRAP
    sdk.require(os.path.isfile(bwrap) and os.access(bwrap, os.X_OK),
                'Bubblewrap is required for immutable SDK consumption')
    # Both reads are pinned. A concurrent manifest replacement cannot swap in
    # different digest rows between validation and the independent sealed copy.
    resource = sdk.validate(manifest_path, compiler=compiler, manifest_sha256=expected_sha256)
    manifest = sdk.read_manifest(manifest_path, expected_sha256=expected_sha256)
    with ExitStack() as snapshots:
        runtime_fds = {}
        for name in sdk.RUNTIMES:
            relative = 'lib/linux/' + name
            fd = sealed_runtime(resource / relative, manifest['resource_files'].get(relative))
            snapshots.callback(os.close, fd)
            runtime_fds[name] = fd
        command = [bwrap, *namespace_arguments(manifest['inputs']['stock_resource'], runtime_fds,
                                              [compiler, *clang_arguments], driver_argv0)]
        # Exec, not a Python supervisor: keep Bubblewrap's documented exit
        # semantics, including 128+signal for a signaled child. These are the only
        # extra descriptors inherited by bwrap; its data options consume/close
        # them rather than preserving them for Clang.
        for fd in runtime_fds.values():
            os.set_inheritable(fd, True)
        # Python starts with these ignored; native subprocess execution restores
        # their defaults too. All other inherited dispositions remain unchanged.
        for name in ('SIGPIPE', 'SIGXFZ', 'SIGXFSZ'):
            if hasattr(signal, name):
                number = getattr(signal, name)
                if signal.getsignal(number) == signal.SIG_IGN:
                    signal.signal(number, signal.SIG_DFL)
        os.execv(bwrap, command)


def main(arguments=None):
    arguments = sys.argv[1:] if arguments is None else arguments
    try:
        sdk.require(len(arguments) >= 3,
                    'expected MANIFEST EXPECTED_MANIFEST_SHA REAL_CLANG [clang arguments...]')
        execute(arguments[0], arguments[1], arguments[2], arguments[3:])
    except (OSError, ValueError) as error:
        print(f'RESOURCE STOP: immutable finite TSan SDK: {error}', file=sys.stderr)
        return 126
    return 126  # A successful execution replaces this process.


if __name__ == '__main__':
    sys.exit(main())
