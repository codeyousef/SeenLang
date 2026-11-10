#!/usr/bin/env python3
"""Validate fresh raw IR and structured reports for tensor bounds fixtures."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def check(ir_dir: Path, log: Path) -> None:
    if ir_dir.is_symlink() or log.is_symlink() or log.stat().st_size > 8 * 1024 * 1024:
        raise ValueError('invalid evidence path or log size')
    files = sorted(ir_dir.glob('*.ll'))
    if not files or len(files) > 128:
        raise ValueError('invalid IR module count')
    names = ['provedTensor', 'shiftedTensor', 'callInLoop', 'updateTensor', 'checkedBulk']
    functions = {}
    for path in files:
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 16 * 1024 * 1024:
            raise ValueError('invalid IR module')
        for match in re.finditer(r'^define [^\n]+\n.*?^}', path.read_text(), re.MULTILINE | re.DOTALL):
            header = match[0].splitlines()[0]
            for name in names:
                if name in header:
                    if name in functions:
                        raise ValueError(f'duplicate fixture function {name}')
                    functions[name] = match[0]
    if set(functions) != set(names):
        raise ValueError('missing raw fixture functions')
    for name in ['provedTensor', 'updateTensor']:
        if 'bounds-proof: canonical zero-to-stable-length read' not in functions[name]:
            raise ValueError(f'missing proof marker in {name}')
        if re.search(r'call void @seen_bounds_check\(', functions[name]):
            raise ValueError(f'proved access still has a frontend bounds guard in {name}')
    if 'bounds-proof: canonical zero-to-stable-length write' not in functions['updateTensor']:
        raise ValueError('missing write proof')
    for name in ['shiftedTensor', 'callInLoop', 'checkedBulk']:
        if not re.search(r'call void @seen_bounds_check\(', functions[name]):
            raise ValueError(f'unproved access lost its frontend bounds guard in {name}')
    records = []
    for line in log.read_text().splitlines():
        if line.startswith('{'):
            value = json.loads(line)
            if isinstance(value, dict) and str(value.get('contract', '')).startswith('seen-simd-'):
                if value.get('hardware_verified') is True or value.get('hardware_certified') is True:
                    raise ValueError('software fixture claimed hardware certification')
                if value.get('truncated') is True:
                    raise ValueError('fixture evidence was truncated')
                records.append(value)
    frontend = [r['loop'] for r in records if r.get('contract') == 'seen-simd-module-record-v1']
    if not any(r.get('bounds') == 'proved' and r.get('checks_eliminated', 0) > 0 for r in frontend):
        raise ValueError('missing applied frontend proof record')
    if not any(r.get('bounds') == 'retained' for r in frontend):
        raise ValueError('missing conservative frontend decision')
    observed = [r for r in records if r.get('phase') == 'llvm-module' and r.get('vectorization') in ['selected', 'rejected']]
    if not observed or any(not r.get('reason') for r in observed):
        raise ValueError('missing observed LLVM decisions/reasons')
    for record in observed:
        if record['vectorization'] == 'selected' and not (0 < record.get('width', 0) <= 65536):
            raise ValueError('invalid observed vector width')
    print('PASS: tensor proof IR, retained guards and observed SIMD JSON evidence')


if __name__ == '__main__':
    try:
        check(Path(sys.argv[1]), Path(sys.argv[2]))
    except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
        raise SystemExit(f'tensor-bounds-evidence: {error}') from error
