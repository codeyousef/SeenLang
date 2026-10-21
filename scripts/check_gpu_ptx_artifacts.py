#!/usr/bin/env python3
"""Independently check PTX launch ABI, hashes, profile parity and assembly."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from check_gpu_ptx_toolchain import invoke, load_profile, verify_installation


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def check_layout(text, reflection):
    entries = list(re.finditer(r'\.visible \.entry ([A-Za-z_][A-Za-z0-9_]*)\(\n([^()]*)\n\)\n\.reqntid (\d+), (\d+), (\d+)\n\{([^{}]*)\n\}', text))
    require(1 <= len(entries) <= 256, 'entry count exceeds the module limit')
    require([match[1] for match in entries] == [entry['name'] for entry in reflection['entries']], 'entry identity differs')
    for match, entry in zip(entries, reflection['entries']):
        require([int(match[i]) for i in (3, 4, 5)] == entry['workgroup'], 'workgroup geometry differs')
        parameters, declarations, offset = [], [], 0

        def add(binding, kind, size, alignment, declaration):
            nonlocal offset
            offset = (offset + alignment - 1) // alignment * alignment
            parameters.append({'index': len(parameters), 'binding': binding, 'kind': kind,
                               'offset': offset, 'bytes': size, 'alignment': alignment})
            declarations.append(declaration)
            offset += size

        for index, binding in enumerate(entry['bindings']):
            require(binding['binding'] == index and binding['group'] == 0, 'logical binding order differs')
            prefix = f'$seen_arg{index}'
            if binding['kind'] == 'uniform':
                add(index, 'uniform_value', 16, 16, f'.param .align 16 .b8 {prefix}_value[16]')
            else:
                require(binding['kind'] == 'storage', 'unknown resource kind')
                add(index, 'storage_address', 8, 8, f'.param .u64 {prefix}_address')
                add(index, 'storage_length', 4, 4, f'.param .u32 {prefix}_length')
        if entry['requires_fault_buffer']:
            add(entry['fault_binding'], 'fault_address', 8, 8, '.param .u64 $seen_fault')
        require(match[2] == ',\n'.join(declarations), 'physical PTX parameter declarations differ')
        require(entry['parameter_abi'] == 'seen-ptx-kernel-v1' and entry['parameters'] == parameters,
                'reflected physical launch plan differs')
        require(entry['parameter_count'] == len(parameters) and entry['packed_parameter_bytes'] == offset <= 4096,
                'packed parameter geometry differs')
        shared = sum(int(size) for size in re.findall(r'\.shared \.align 16 \.b8 \$seen_shared\d+\[(\d+)\];', match[6]))
        require(shared == entry['shared_bytes'] <= 49152, 'shared byte count differs')
        keys = ('workgroup', 'bindings', 'required_capabilities', 'shared_bytes', 'requires_fault_buffer',
                'fault_binding', 'parameter_abi', 'packed_parameter_bytes', 'parameters', 'parameter_count')
        abi = {key: entry[key] for key in keys}
        abi['specializations'] = reflection['specializations']
        canonical = json.dumps(abi, separators=(',', ':'), ensure_ascii=False).encode()
        require(digest(canonical) == entry['abi_sha256'], 'entry ABI digest differs')


def check_artifact(record, reflection, manifest):
    target, text = record['target'], record['text']
    version = '8.7' if target == 'sm_100' else '8.0'
    require(text.startswith(f'.version {version}\n.target {target}\n.address_size 64\n'), 'PTX target header differs')
    require(reflection['backend'] == manifest['backend'] == 'cuda' and reflection['target'] == manifest['target'] == target,
            'backend/target identity differs')
    require(manifest['classification'] == 'compile-only' and manifest['official_validation'] == 'not-run', 'invalid artifact classification')
    require(manifest['byte_order'] == 'utf-8' and manifest['word_count'] == 0 and
            manifest['parameter_abi'] == 'seen-ptx-kernel-v1', 'invalid PTX artifact encoding or ABI')
    for document in (reflection, manifest):
        require(document['hardware_verified'] is False and document['gpu_execution_verified'] is False, 'offline artifact claims execution')
    raw = text.encode('utf-8')
    require(manifest['byte_count'] == len(raw) <= 33554432 and manifest['sha256'] == digest(raw), 'PTX byte hash differs')
    require(manifest['reflection_sha256'] == digest(json.dumps(reflection, separators=(',', ':'), ensure_ascii=False).encode()),
            'reflection hash differs')
    check_layout(text, reflection)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('names', nargs='+')
    args = parser.parse_args()
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
            require(not pending and targets == toolchain['targets'], 'required target coverage differs')
        if baseline is None: baseline = records
        require(records == baseline, 'PTX artifact or reflection differs across compiler profiles')
    evidence = {'schema': 'seen-ptx-validation-v1', 'official_assembly': True, 'device_execution': False,
                'toolchain': identity, 'profiles': ['fast', 'release', 'undefined', 'address']}
    (args.root / 'validation.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: Seen-emitted PTX passes official assembly, launch ABI, hashes and four-profile identity')


if __name__ == '__main__':
    main()
