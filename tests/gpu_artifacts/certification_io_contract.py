#!/usr/bin/env python3
"""Exercise bounded artifact snapshots against real filesystem adverse cases."""
import os
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from gpu_certification_io import read_bounded, invoke_with_snapshot, prepare_output


def rejected(path, ceiling):
    try:
        read_bounded(path, ceiling)
    except ValueError:
        return
    raise AssertionError('unsafe or oversized snapshot was accepted')


with tempfile.TemporaryDirectory(prefix='artifact-read.', dir=sys.argv[1]) as temporary:
    root = Path(temporary)
    source = root / 'source'
    source.write_bytes(b'valid')
    assert read_bounded(source, 5) == b'valid'
    assert invoke_with_snapshot(source, 5, lambda: 'result', b'valid') == 'result'
    invoked = []
    try:
        invoke_with_snapshot(source, 5, lambda: invoked.append(True), b'other')
        raise AssertionError('a tool received mismatched input')
    except ValueError:
        assert not invoked
    try:
        invoke_with_snapshot(source, 5, lambda: source.write_bytes(b'other'))
        raise AssertionError('a tool input mutation was accepted')
    except ValueError:
        pass
    source.write_bytes(b'valid')
    link = root / 'link'
    link.symlink_to(source)
    unsafe = [root, link]
    if hasattr(os, 'mkfifo'):
        fifo = root / 'fifo'
        os.mkfifo(fifo)
        unsafe.append(fifo)
    descriptors = Path('/proc/self/fd')
    before = len(list(descriptors.iterdir())) if descriptors.is_dir() else None
    for _ in range(100):
        assert read_bounded(source, 5) == b'valid'
        rejected(source, 4)
        for path in unsafe:
            rejected(path, 5)
    if before is not None:
        assert len(list(descriptors.iterdir())) == before, 'snapshot rejection leaked descriptors'
    output = root / 'output'
    prepare_output(output); assert not output.exists()
    output.write_bytes(b'generated'); prepare_output(output); assert not output.exists()
    for path in unsafe:
        try:
            prepare_output(path)
        except ValueError:
            pass
        else:
            raise AssertionError('unsafe generated output was accepted')
    assert read_bounded(source, 5) == b'valid' and link.is_symlink()
print('PASS: GPU certification snapshots reject oversized symlink directory and FIFO inputs without descriptor leaks')
