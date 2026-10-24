#!/usr/bin/env python3
"""Check MSL source integrity and native buffer reflection, without SDK claims."""
import argparse
import json
from pathlib import Path
import re

from gpu_certification_io import read_bounded, prepare_output
from gpu_launch_contract import require, digest, check_text_manifest, check_abi_digest

TARGETS = {'macos13': 'metal3.0', 'macos14': 'metal3.1', 'macos15': 'metal3.2'}
PROFILES = ('fast', 'release', 'undefined', 'address')
ABI_KEYS = ('msl_version', 'requires_apple_silicon', 'buffer_slot_ceiling', 'source_parameter_ceiling',
            'buffer_offset_alignment', 'fault_buffer_bytes', 'launch_binding', 'launch_bytes',
            'launch_alignment', 'launch_fault_mask', 'entry_name_prefix', 'launch_counts')
ATOMIC_POINTER_CAST = re.compile(
    r'reinterpret_cast<(?P<const>const )?(?P<space>device|threadgroup) atomic_(?P<kind>int|uint)\*>\( &'
    r'seen_(?:buffer|shared)[0-9]+\[ulong\(seen_w[1-9][0-9]*\) \* 1ul \+ 0ul\]\)(?=,)')


def check_artifact(record, reflection, manifest):
    target, text = record['target'], record['text']
    require(target in TARGETS and len(text) <= 33554432, 'invalid MSL target or source size')
    check_text_manifest(record, reflection, manifest, 'metal', 'seen-metal-kernel-v1')
    raw = text.encode('ascii')
    require(text.startswith('// seen-metal-kernel-v1; compile-only;') and
            '// msl-version: ' + TARGETS[target] + '\n' in text, 'MSL header differs')
    admitted = ATOMIC_POINTER_CAST.sub('seen_atomic_address', text)
    require(not re.search(r'\b(goto|reinterpret_cast|new|delete|asm)\b', admitted), 'unexpected MSL construct')
    entries = reflection['entries']
    require(entries and len(entries) <= 256, 'invalid Metal entry count')
    names = re.findall(r'\nkernel void (seen_metal_entry_[A-Za-z0-9_]+)\(', text)
    require(names == ['seen_metal_entry_' + entry['name'] for entry in entries], 'Metal native entry names differ')
    for entry in entries:
        check_abi_digest(entry, reflection['specializations'], ABI_KEYS, packed=False)
        bindings = entry['bindings']; count = len(bindings)
        require(count <= 29 and entry['requires_fault_buffer'] is True and entry['fault_binding'] == count,
                'Metal fault buffer binding differs')
        require(entry['parameter_abi'] == 'seen-metal-kernel-v1' and entry['msl_version'] == TARGETS[target] and
                entry['requires_apple_silicon'] is (target == 'macos15'), 'Metal target ABI differs')
        require(entry['buffer_slot_ceiling'] == 31 and entry['source_parameter_ceiling'] == 29 and
                entry['buffer_offset_alignment'] == 256 and entry['fault_buffer_bytes'] == 4 and
                entry['launch_alignment'] == 16 and entry['launch_fault_mask'] == 16 and
                entry['entry_name_prefix'] == 'seen_metal_entry_', 'Metal ABI limits differ')
        require(entry['launch_binding'] == count + 1 and entry['launch_bytes'] == max(16, count * 16),
                'Metal launch count buffer differs')
        expected = []
        for index, binding in enumerate(bindings):
            require(binding['binding'] == index and binding['group'] == 0, 'Metal source binding differs')
            lanes = binding['lanes']
            require(binding['element_stride'] == (16 if lanes == 3 else lanes * 4), 'Metal vector stride differs')
            if binding['kind'] == 'uniform':
                require(binding['minimum_binding_bytes'] == 16, 'Metal uniform payload differs')
            else:
                expected.append({'binding': index, 'offset': index * 16, 'bytes': 4, 'unit': 'elements'})
        require(entry['launch_counts'] == expected, 'Metal logical-count offsets differ')
    return raw


def collect(root, names, profiles=PROFILES, *, msl32_only=()):
    require(root.is_dir() and not root.is_symlink(), 'invalid Metal evidence directory')
    require(profiles and len(set(profiles)) == len(profiles) and set(profiles) <= set(PROFILES), 'invalid Metal compiler profiles')
    require(set(msl32_only) <= set(names), 'unknown MSL 3.2 fixture')
    records = []
    for name in names:
        require(re.fullmatch('[a-z][a-z0-9-]{0,63}', name) is not None, 'invalid Metal fixture name')
        targets = ('macos15',) if name in msl32_only else tuple(TARGETS)
        baseline = {}
        for profile in profiles:
            path = root / f'{name}-{profile}.log'
            lines = read_bounded(path, 67108864).decode('utf-8').splitlines()
            documents = [json.loads(line) for line in lines if line.startswith('{"schema":')]
            require(len(documents) == len(targets) * 3, 'Metal target artifact/reflection/manifest coverage differs')
            for index, target in enumerate(targets):
                record, reflection, manifest = documents[index * 3:index * 3 + 3]
                require(record['schema'] == 'seen-metal-artifact-v1' and record['target'] == target,
                        'Metal target order differs')
                raw = check_artifact(record, reflection, manifest)
                snapshot = (raw, reflection, manifest)
                if profile == profiles[0]: baseline[target] = snapshot
                else: require(snapshot == baseline[target], 'Metal artifact differs across compiler profiles')
                prefix = root / f'{name}-{profile}-{target}'
                prepare_output(prefix.with_suffix('.metal')); prefix.with_suffix('.metal').write_bytes(raw)
                for suffix, value in (('.json', reflection), ('.manifest.json', manifest)):
                    prepare_output(prefix.with_suffix(suffix))
                    prefix.with_suffix(suffix).write_text(json.dumps(value, separators=(',', ':')) + '\n')
                records.append({'fixture': name, 'profile': profile, 'target': target, 'sha256': digest(raw)})
    return records


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path); parser.add_argument('names', nargs='+')
    parser.add_argument('--msl32-only', nargs='*', default=[])
    args = parser.parse_args(); records = collect(args.root, args.names, msl32_only=args.msl32_only)
    print(json.dumps({'schema': 'seen-metal-source-check-v1', 'records': records,
                      'official_validation': 'not-run', 'gpu_execution_verified': False}, sort_keys=True))
