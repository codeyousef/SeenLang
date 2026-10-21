"""Shared identity and resource limits for offline GPU tool invocations."""
import hashlib
import os
from pathlib import Path
import resource
import subprocess
import tempfile


class ToolProcessError(ValueError):
    def __init__(self, message, *, timeout=False):
        super().__init__(message)
        self.timeout = timeout


def require(value, message):
    if not value:
        raise ToolProcessError(message)


def memory_budget():
    available = next(int(line.split()[1]) * 1024 for line in Path('/proc/meminfo').read_text().splitlines()
                     if line.startswith('MemAvailable:'))
    maximum_path = Path('/sys/fs/cgroup/memory.max')
    if maximum_path.is_file():
        maximum = maximum_path.read_text().strip()
        if maximum != 'max':
            available = min(available, max(0, int(maximum) - int(Path('/sys/fs/cgroup/memory.current').read_text())))
    budget = min(2 * 1024**3, available // 4)
    require(budget >= 256 * 1024**2, 'insufficient validator memory headroom')
    return budget


def identity(path):
    info = path.stat()
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def inspect_tool(path, expected):
    require(path.is_file() and not path.is_symlink() and os.access(path, os.X_OK), 'official executable is unavailable')
    before = identity(path)
    require(before[2] <= 64 * 1024**2, 'tool binary exceeds byte ceiling')
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    require(identity(path) == before and digest == expected, 'official executable digest differs')
    return before


def invoke(tool, arguments, output_limit=65536):
    path = Path(tool['path'])
    before = inspect_tool(path, tool['sha256'])
    budget = memory_budget()

    def limits():
        resource.setrlimit(resource.RLIMIT_AS, (budget, budget))
        resource.setrlimit(resource.RLIMIT_CPU, (30, 30))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        resource.setrlimit(resource.RLIMIT_FSIZE, (64 * 1024**2, 64 * 1024**2))

    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            result = subprocess.run([str(path), *arguments], stdout=stdout, stderr=stderr,
                                    timeout=45, env={'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'}, preexec_fn=limits)
        except subprocess.TimeoutExpired as error:
            raise ToolProcessError('official tool exceeded its deadline', timeout=True) from error
        require(identity(path) == before, 'official executable changed during invocation')
        require(stdout.tell() <= output_limit and stderr.tell() <= output_limit, 'official tool diagnostic output exceeds ceiling')
        stdout.seek(0); stderr.seek(0)
        output, diagnostic = stdout.read().decode('utf-8', 'replace'), stderr.read().decode('utf-8', 'replace')
    require(result.returncode >= 0, 'official tool terminated by a signal')
    return result.returncode, output, diagnostic
