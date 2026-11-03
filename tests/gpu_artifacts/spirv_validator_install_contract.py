#!/usr/bin/env python3
"""Refuse invalid SDK archives without altering the installed image or leaking FDs."""
import os
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from install_gpu_spirv_validator import MAX_ARCHIVE_BYTES, install


def refused(destination, archive):
    try:
        install(destination, archive)
    except (OSError, ValueError):
        return
    raise AssertionError('unsafe vendor archive or destination was accepted')


with tempfile.TemporaryDirectory(prefix='spirv-validator-install.', dir=sys.argv[1]) as temporary:
    root = Path(temporary).resolve()
    destination = root / 'libSPIRV-Tools-shared.so'
    prior = b'previous-installed-image'
    destination.write_bytes(prior)
    damaged = root / 'damaged.a'
    damaged.write_bytes(b'!<arch>\nwrong-vendor-bytes')
    empty = root / 'empty.a'
    empty.touch()
    oversized = root / 'oversized.a'
    with oversized.open('wb') as output:
        output.truncate(MAX_ARCHIVE_BYTES + 1)
    symlink = root / 'linked.a'
    symlink.symlink_to(damaged)
    fifo = root / 'fifo.a'
    os.mkfifo(fifo)
    before = len(list(Path('/proc/self/fd').iterdir()))
    for _ in range(100):
        for archive in (damaged, empty, oversized, symlink, fifo, root, root / 'absent.a'):
            refused(destination, archive)
            assert destination.read_bytes() == prior, 'refusal replaced the previous image'
        assert not list(root.glob('.spirv-validator-*')), 'refusal retained an installer work directory'
    assert len(list(Path('/proc/self/fd').iterdir())) == before, 'archive refusal leaked descriptors'
    destination.unlink()
    destination.symlink_to(damaged)
    refused(destination, damaged)
    assert destination.is_symlink() and damaged.read_bytes() == b'!<arch>\nwrong-vendor-bytes'
    destination.unlink()
    destination.mkdir()
    refused(destination, damaged)
    assert destination.is_dir(), 'refusal replaced a destination directory'
print('PASS: pinned validator installation rejects damaged oversized special-file and symlink inputs; prior image and FDs preserved')
