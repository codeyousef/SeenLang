"""Bounded immutable file snapshots for offline GPU certification tools."""
import json
import os
import stat
from pathlib import Path


def require(value, message):
    if not value:
        raise ValueError(message)


def prepare_output(path):
    """Reject links and special files before replacing a generated output."""
    if path.exists() or path.is_symlink():
        require(path.is_file() and not path.is_symlink(), 'unsafe GPU output destination')
        path.unlink()


def read_bounded(path, ceiling):
    path = Path(path)
    entry = path.lstat()
    require(stat.S_ISREG(entry.st_mode), 'artifact is not a regular file')
    descriptor = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0))
    with os.fdopen(descriptor, 'rb') as source:
        before = os.fstat(source.fileno())
        require((entry.st_dev, entry.st_ino) == (before.st_dev, before.st_ino) and
                stat.S_ISREG(before.st_mode) and 0 <= before.st_size <= ceiling,
                'artifact unavailable or oversized')
        content = source.read(ceiling + 1)
        after = os.fstat(source.fileno())
        identity = lambda value: (value.st_dev, value.st_ino, value.st_size,
                                  value.st_mtime_ns, value.st_ctime_ns)
        require(identity(before) == identity(after) and len(content) == before.st_size <= ceiling,
                'artifact changed while reading')
        return content


def manifest_from_log(path):
    lines = read_bounded(path, 1048576).decode().splitlines()
    records = [json.loads(line) for line in lines
               if line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"')]
    require(len(records) == 1, 'expected exactly one compiler integrity manifest')
    return records[0]


def invoke_with_snapshot(path, ceiling, invocation, expected=None):
    before = read_bounded(path, ceiling)
    require(expected is None or before == expected, 'tool input differs from its verified snapshot')
    result = invocation()
    require(read_bounded(path, ceiling) == before, 'tool input changed during invocation')
    return result
