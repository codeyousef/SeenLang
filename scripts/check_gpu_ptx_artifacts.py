#!/usr/bin/env python3
"""Independently check PTX launch ABI, hashes, profile parity and assembly."""
import argparse
import json
from pathlib import Path
import re
from check_gpu_ptx_toolchain import invoke, load_profile, verify_installation


from gpu_launch_contract import require, digest, physical_parameters, check_abi_digest, check_text_manifest


def check_layout(text, reflection):
    entries = list(re.finditer(r'\.visible \.entry ([A-Za-z_][A-Za-z0-9_]*)\(\n([^()]*)\n\)\n\.reqntid (\d+), (\d+), (\d+)\n\{((?:[^{}]|\{[^{}]*\})*)\n\}', text))
    require(1 <= len(entries) <= 256, 'entry count exceeds the module limit')
    require(text.count('.visible .entry ') == len(entries), 'unparsed PTX entry declaration')
    require([match[1] for match in entries] == [entry['name'] for entry in reflection['entries']], 'entry identity differs')
    for match, entry in zip(entries, reflection['entries']):
        require([int(match[i]) for i in (3, 4, 5)] == entry['workgroup'], 'workgroup geometry differs')
        parameters, _ = physical_parameters(entry, 'seen-ptx-kernel-v1')
        declarations = []
        for parameter in parameters:
            prefix = f"$seen_arg{parameter['binding']}"
            kind = parameter['kind']
            if kind == 'uniform_value': declarations.append(f'.param .align 16 .b8 {prefix}_value[16]')
            elif kind == 'storage_address': declarations.append(f'.param .u64 {prefix}_address')
            elif kind == 'storage_length': declarations.append(f'.param .u32 {prefix}_length')
            else: declarations.append('.param .u64 $seen_fault')
        require(match[2] == ',\n'.join(declarations), 'physical PTX parameter declarations differ')
        shared = sum(int(size) for size in re.findall(r'\.shared \.align 16 \.b8 \$seen_shared\d+\[(\d+)\];', match[6]))
        require(shared == entry['shared_bytes'] <= 49152, 'shared byte count differs')
        check_abi_digest(entry, reflection['specializations'])


def check_artifact(record, reflection, manifest):
    target, text = record['target'], record['text']
    version = '8.7' if target == 'sm_100' else '8.0'
    require(text.startswith(f'.version {version}\n.target {target}\n.address_size 64\n'), 'PTX target header differs')
    check_text_manifest(record, reflection, manifest, 'cuda', 'seen-ptx-kernel-v1')
    check_layout(text, reflection)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('names', nargs='+')
    parser.add_argument('--sm80-minimum', action='append', default=[], metavar='FIXTURE',
                        help='require all pinned sm_80+ targets for this fixture')
    args = parser.parse_args()
    require(set(args.sm80_minimum) <= set(args.names), 'target floor refers to an unknown fixture')
    toolchain = load_profile(); identity = verify_installation(toolchain)
    baseline = None
    for profile in ('fast', 'release', 'undefined', 'address'):
        records = []
        for name in args.names:
            path = args.root / f'{name}-{profile}.log'
            require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 64 * 1024**2, 'invalid fixture log')
            pending, targets = [], []
            for line in path.read_text().splitlines():
                if line.startswith('{"schema":"seen-ptx-artifact-v1"'):
                    require(not pending, 'artifact lacks paired metadata'); pending.append(json.loads(line))
                elif line.startswith('{"schema":"seen-gpu-reflection-v1"'):
                    require(len(pending) == 1, 'reflection lacks paired artifact'); pending.append(json.loads(line))
                elif line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"'):
                    require(len(pending) == 2, 'manifest lacks paired artifact/reflection')
                    manifest = json.loads(line); record, reflection = pending; pending = []
                    check_artifact(record, reflection, manifest)
                    target = record['target']; targets.append(target)
                    source = args.root / f'{name}-{profile}-{target}.ptx'
                    source.write_text(record['text'])
                    output = source.with_suffix('.cubin')
                    status, stdout, diagnostic = invoke(toolchain, 'ptxas', ['-arch=' + target, str(source), '-o', str(output)])
                    require(status == 0 and not stdout and not diagnostic and output.is_file() and output.stat().st_size > 0,
                            'official assembly failed: ' + diagnostic)
                    records.append([record, reflection, manifest])
            expected_targets = toolchain['targets']
            if name in args.sm80_minimum:
                expected_targets = expected_targets[expected_targets.index('sm_80'):]
            require(not pending and targets == expected_targets, 'required target coverage differs')
        if baseline is None: baseline = records
        require(records == baseline, 'PTX artifact or reflection differs across compiler profiles')
    evidence = {'schema': 'seen-ptx-validation-v1', 'official_assembly': True, 'device_execution': False,
                'toolchain': identity, 'profiles': ['fast', 'release', 'undefined', 'address']}
    (args.root / 'validation.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: Seen-emitted PTX passes official assembly, launch ABI, hashes and four-profile identity')


if __name__ == '__main__':
    main()
