#!/usr/bin/env python3
"""Adverse MSL metadata checks after re-signing intentionally malformed data."""
import copy
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from check_gpu_metal_artifacts import check_artifact, ABI_KEYS, ATOMIC_POINTER_CAST
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


def load_artifact(source):
    reflection = json.loads(read_bounded(source.with_suffix('.json'), 4194304))
    return ({'text': read_bounded(source, 33554432).decode(), 'target': reflection['target']}, reflection,
            json.loads(read_bounded(source.with_suffix('.manifest.json'), 1048576)))


def reject_mutations(original, mutations):
    for mutate in mutations:
        record, reflection, manifest = copy.deepcopy(original)
        mutate(record, reflection, manifest); resign(record, reflection, manifest)
        try: check_artifact(record, reflection, manifest)
        except ValueError: pass
        else: raise ValueError('malformed Metal metadata was accepted')


def atomic_casts(source):
    original = load_artifact(source); check_artifact(*original)
    match = ATOMIC_POINTER_CAST.search(original[0]['text']); require(match is not None, 'missing native atomic cast witness')
    cast = match.group(0)
    bad = [cast.replace('atomic_int', 'atomic_float'), cast.replace('device ', 'constant '),
           cast.replace('1ul + 0ul', '2ul + 0ul'), cast.replace('1ul + 0ul', '1ul + 1ul'),
           cast.replace('&seen_buffer', '&foreign_buffer'), cast.replace('reinterpret_cast<', 'reinterpret_cast<volatile '),
           cast + ' + 1']
    require(len(set(bad)) == 7 and cast not in bad, 'missing distinct adverse native casts')
    mutations = [lambda r, f, m, value=value: r.update(text=r['text'].replace(cast, value, 1)) for value in bad]
    reject_mutations(original, mutations)
    print('PASS: Metal atomic pointers reject seven re-signed type space stride offset and address corruptions')


def main(root):
    original = load_artifact(root / 'layout-fast-macos13.metal')
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
    reject_mutations(original, mutations)
    require(len(mutations) == 12, 'missing Metal adverse cases')
    print('PASS: Metal source ABI bounds and verification claims reject re-signed corrupt metadata')


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--atomic': atomic_casts(Path(sys.argv[2]))
    else: main(Path(sys.argv[1]))
