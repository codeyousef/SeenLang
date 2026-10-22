#!/usr/bin/env python3
"""Adverse MSL metadata checks after re-signing intentionally malformed data."""
import copy
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from check_gpu_metal_artifacts import check_artifact, ABI_KEYS
from gpu_launch_contract import digest, require
from gpu_certification_io import read_bounded


def resign(record, reflection, manifest):
    for entry in reflection['entries']:
        keys = ('workgroup', 'bindings', 'required_capabilities', 'shared_bytes',
                'requires_fault_buffer', 'fault_binding', 'parameter_abi') + ABI_KEYS
        abi = {key: entry[key] for key in keys}; abi['specializations'] = reflection['specializations']
        entry['abi_sha256'] = digest(json.dumps(abi, separators=(',', ':')).encode())
    raw = record['text'].encode(); manifest['byte_count'] = len(raw); manifest['sha256'] = digest(raw)
    manifest['reflection_sha256'] = digest(json.dumps(reflection, separators=(',', ':')).encode())


def main(root):
    source = root / 'layout-fast-macos13.metal'
    original = ({'text': read_bounded(source, 33554432).decode(), 'target': 'macos13'},
                json.loads(read_bounded(source.with_suffix('.json'), 4194304)),
                json.loads(read_bounded(source.with_suffix('.manifest.json'), 1048576)))
    check_artifact(*original)
    mutations = [
        lambda r, f, m: r.update(text=r['text'] + '\ngoto forged;\n'),
        lambda r, f, m: r.update(text=r['text'] + '\x00'),
        lambda r, f, m: r.update(text=r['text'].replace('metal3.0', 'metal3.1')),
        lambda r, f, m: r.update(text=r['text'].replace('seen_metal_entry_integers(', 'seen_metal_entry_forged(')),
        lambda r, f, m: f['entries'][0].update(launch_binding=0),
        lambda r, f, m: f['entries'][0].update(launch_bytes=1),
        lambda r, f, m: f['entries'][0].update(fault_binding=0),
        lambda r, f, m: f['entries'][0]['launch_counts'][0].update(unit='bytes'),
        lambda r, f, m: f['entries'][1]['bindings'][0].update(element_stride=12),
        lambda r, f, m: f['entries'][0].update(requires_apple_silicon=True),
        lambda r, f, m: m.update(official_validation='passed'),
        lambda r, f, m: m.update(gpu_execution_verified=True),
    ]
    for mutate in mutations:
        record, reflection, manifest = copy.deepcopy(original)
        mutate(record, reflection, manifest); resign(record, reflection, manifest)
        try: check_artifact(record, reflection, manifest)
        except ValueError: pass
        else: raise ValueError('malformed Metal metadata was accepted')
    require(len(mutations) == 12, 'missing Metal adverse cases')
    print('PASS: Metal source ABI bounds and verification claims reject re-signed corrupt metadata')


if __name__ == '__main__': main(Path(sys.argv[1]))
