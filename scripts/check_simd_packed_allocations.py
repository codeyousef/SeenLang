#!/usr/bin/env python3
"""Check raw kernel IR so packed hot loops cannot hide boxed lane allocations."""
import re
import sys
from pathlib import Path


def check(directory):
    files = sorted(directory.glob('*.ll'))
    if directory.is_symlink() or not 1 <= len(files) <= 128:
        raise ValueError('invalid packed-kernel IR directory')
    names = {'unsafePackedGet', 'packedSet', 'packedIndexGeometryValid', 'transformPackedSimdBytes'}
    bodies = {}
    for path in files:
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 16 * 1024**2:
            raise ValueError('invalid packed-kernel IR module')
        text = path.read_text()
        for match in re.finditer(r'^define [^\n]*@([A-Za-z_][A-Za-z0-9_]*)\([^\n]*\)[^\n]*\{\n.*?^}', text, re.M | re.S):
            name = match[1]
            if name in names:
                if name in bodies:
                    raise ValueError('duplicate packed-kernel function: ' + name)
                bodies[name] = match[0]
    if set(bodies) != names:
        raise ValueError('missing packed-kernel functions: ' + ', '.join(sorted(names - set(bodies))))
    def callees(body):
        calls = re.findall(r'^\s*(?:%[^=\n]+=\s*)?(?:(?:tail|musttail|notail)\s+)?(?:call|invoke)\b([^\n]*)', body, re.M)
        result = set()
        for instruction in calls:
            target = re.search(r'@([^\s(]+)\(', instruction)
            if target is None:
                raise ValueError('indirect call in packed-kernel evidence')
            result.add(target[1])
        return result
    allowed = {'unsafePackedGet': set(), 'packedSet': {'packedIndexGeometryValid'},
               'packedIndexGeometryValid': set()}
    for name, allowed_callees in allowed.items():
        actual = callees(bodies[name])
        if not actual <= allowed_callees:
            raise ValueError('packed scalar helper calls an allocator or unproved helper: ' + name)
    calls = callees(bodies['transformPackedSimdBytes'])
    if 'unsafePackedGet' not in calls or 'packedGet' in calls:
        raise ValueError('bulk unpacking omitted the allocation-free prevalidated read')
    print('PASS: packed lane helpers have no allocating callees and bulk unpacking avoids boxed Options')


if __name__ == '__main__':
    try:
        check(Path(sys.argv[1]))
    except (OSError, UnicodeError, ValueError, IndexError) as error:
        raise SystemExit('simd-packed-allocations: ' + str(error)) from error
