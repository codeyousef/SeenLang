#!/usr/bin/env python3
"""Check emitted LLVM, independently reflected HSA ABI and official code objects."""
import argparse
import json
import math
from pathlib import Path
import re
import struct

from check_gpu_amdgpu_toolchain import invoke, load_profile, verify_installation
from gpu_certification_io import read_bounded, invoke_with_snapshot, prepare_output
from gpu_launch_contract import require, digest, physical_parameters, check_abi_digest, check_text_manifest

EXTRA_ABI_KEYS = ('hsa_code_object_version', 'kernarg_alignment', 'implicit_argument_offset', 'implicit_argument_bytes',
                  'kernel_argument_bytes', 'wavefront_size', 'static_shared_byte_ceiling', 'launch_fault_mask')
HIDDEN_ARGUMENTS = {
    **{f'hidden_block_count_{axis}': (i * 4, 4) for i, axis in enumerate('xyz')},
    **{f'hidden_group_size_{axis}': (12 + i * 2, 2) for i, axis in enumerate('xyz')},
    **{f'hidden_remainder_{axis}': (18 + i * 2, 2) for i, axis in enumerate('xyz')},
    **{f'hidden_global_offset_{axis}': (40 + i * 8, 8) for i, axis in enumerate('xyz')},
    'hidden_grid_dims': (64, 2), 'hidden_hostcall_buffer': (80, 8), 'hidden_multigrid_sync_arg': (88, 8),
    'hidden_heap_v1': (96, 8), 'hidden_default_queue': (104, 8), 'hidden_completion_action': (112, 8), 'hidden_queue_ptr': (200, 8),
}


def parameter_name(parameter):
    kind = parameter['kind']
    if kind == 'fault_address': return 'seen_fault'
    suffix = {'uniform_value': 'value', 'storage_address': 'address', 'storage_length': 'length'}[kind]
    return f"arg{parameter['binding']}_{suffix}"


def check_artifact(record, reflection, manifest):
    check_text_manifest(record, reflection, manifest, 'hip', 'seen-amdgpu-kernel-v1')
    text, target = record['text'], record['target']
    require(text.startswith('; seen-amdgpu-kernel-v1; HSA code-object-v5;') and
            'source_filename = "seen-amdgpu-kernel-v1"' in text and
            'target triple = "amdgcn-amd-amdhsa"' in text, 'AMDGPU target header differs')
    entries = list(re.finditer(r'^define amdgpu_kernel void @([A-Za-z_][A-Za-z0-9_]*)\((.*)\) #(\d+) !reqd_work_group_size !(\d+) \{', text, re.M))
    require(1 <= len(entries) <= 256 and text.count('\ndefine amdgpu_kernel void @') == len(entries), 'AMDGPU entry count differs')
    require([item[1] for item in entries] == [entry['name'] for entry in reflection['entries']], 'AMDGPU entry identity differs')
    for match, entry in zip(entries, reflection['entries']):
        parameters, explicit_bytes = physical_parameters(entry, 'seen-amdgpu-kernel-v1')
        declarations = []
        for parameter in parameters:
            kind = parameter['kind']
            declaration = '<4 x i32>' if kind == 'uniform_value' else ('i32' if kind == 'storage_length' else 'ptr addrspace(1)')
            declarations.append(declaration + ' %' + parameter_name(parameter))
        require(match[2] == ', '.join(declarations) and int(match[3]) == int(match[4]) == entry['function'], 'AMDGPU physical declaration differs')
        require(entry['requires_fault_buffer'] is True and entry['launch_fault_mask'] == 16, 'AMDGPU launch guard lacks its fault binding')
        implicit = (explicit_bytes + 7) // 8 * 8
        require(entry['hsa_code_object_version'] == 5 and entry['kernarg_alignment'] == 16 and
                entry['implicit_argument_offset'] == implicit and entry['implicit_argument_bytes'] == 256 and
                entry['kernel_argument_bytes'] == implicit + 256 <= 4096, 'HSA implicit argument plan differs')
        wave = 64 if target in ('gfx90a', 'gfx942') else 32
        require(entry['wavefront_size'] == wave, 'reflected wavefront size differs')
        group = entry['function']
        attribute = re.search(r'^attributes #' + str(group) + r' = \{ ([^\n]+) \}$', text, re.M)
        require(attribute is not None, 'AMDGPU function attributes omitted')
        attributes = dict(re.findall(r'"([^"]+)"="([^"]*)"', attribute[1]))
        size = math.prod(entry['workgroup'])
        require(attributes == {'target-cpu': target, 'target-features': ('+wavefrontsize64,-wavefrontsize32' if wave == 64 else '+wavefrontsize32,-wavefrontsize64'),
                               'amdgpu-flat-work-group-size': f'{size},{size}', 'denormal-fp-math-f32': 'ieee,ieee'}, 'AMDGPU attributes differ')
        dimensions = ', '.join('i32 ' + str(value) for value in entry['workgroup'])
        require(f'!{group} = !{{{dimensions}}}' in text, 'AMDGPU workgroup metadata differs')
        shared = sum(map(int, re.findall(r'^@seen\.shared\.' + str(group) + r'\.\d+ = internal addrspace\(3\) global \[(\d+) x i8\] undef, align 16$', text, re.M)))
        require(entry['shared_bytes'] <= shared == entry['static_shared_byte_ceiling'] <= 65536 and shared % 16 == 0, 'static shared-storage ceiling differs')
        check_abi_digest(entry, reflection['specializations'], EXTRA_ABI_KEYS)


def unique_fields(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate official metadata field')
        result[key] = value.strip()
    return result


def fields(text):
    return unique_fields(re.findall(r'^\s*(?:- )?\.(\w+):\s*([^\n]+)$', text, re.M))


def check_metadata(document, reflection, profile):
    require(len(document) == 1 and document[0]['FileSummary']['Arch'] == 'amdgcn', 'official object architecture differs')
    notes = [note for section in document[0]['NoteSections'] for note in section['NoteSection']['Notes']
             if note.get('Owner') == 'AMDGPU' and 'AMDGPU Metadata' in note]
    require(len(notes) == 1, 'expected one official HSA metadata note')
    text = notes[0]['AMDGPU Metadata']
    require(len(text.encode()) <= 8 * 1024**2 and
            re.findall(r'^amdhsa\.target:\s+(\S+)$', text, re.M) == ['amdgcn-amd-amdhsa--' + reflection['target']],
            'HSA target metadata differs')
    section = re.search(r'^amdhsa\.kernels:\n(.*?)(?=^amdhsa\.|\Z)', text, re.M | re.S)
    require(section is not None, 'HSA kernel metadata omitted')
    blocks = re.findall(r'^  - .*?(?=^  - |\Z)', section[1], re.M | re.S)
    kernels = {}
    for block in blocks:
        name = re.search(r'^    \.name:\s+(\S+)$', block, re.M)
        require(name is not None and name[1] not in kernels, 'invalid HSA kernel identity')
        kernels[name[1]] = block
    require(set(kernels) == {entry['name'] for entry in reflection['entries']}, 'official kernel set differs')
    for entry in reflection['entries']:
        block = kernels[entry['name']]
        scalar = {key: int(value) for key, value in unique_fields(re.findall(r'^    \.(\w+):\s+(\d+)$', block, re.M)).items()}
        require(scalar['kernarg_segment_size'] == entry['kernel_argument_bytes'] and
                1 <= scalar['kernarg_segment_align'] <= 16 and 16 % scalar['kernarg_segment_align'] == 0,
                'official kernarg geometry differs')
        require(scalar['wavefront_size'] == entry['wavefront_size'] and scalar['max_flat_workgroup_size'] == math.prod(entry['workgroup']),
                'official wavefront/workgroup geometry differs')
        require(scalar['group_segment_fixed_size'] <= entry['static_shared_byte_ceiling'] and
                scalar['private_segment_fixed_size'] <= profile['maximum_private_bytes_per_thread'], 'official storage exceeds its ceiling')
        required = re.search(r'^    \.reqd_workgroup_size:\n((?:      - \d+\n){3})', block, re.M)
        require(required is not None and list(map(int, re.findall(r'\d+', required[1]))) == entry['workgroup'], 'official required workgroup differs')
        argument_section = re.search(r'^(?:    |  - )\.args:\n(.*?)(?=^    \.|\Z)', block, re.M | re.S)
        require(argument_section is not None, 'official argument metadata omitted')
        arguments = [fields(item) for item in re.findall(r'^      - .*?(?=^      - |\Z)', argument_section[1], re.M | re.S)]
        explicit = entry['parameters']
        require(len(arguments) >= len(explicit), 'official arguments omitted')
        for actual, expected in zip(arguments, explicit):
            require(actual.get('name') == parameter_name(expected) and int(actual['offset']) == expected['offset'] and
                    int(actual['size']) == expected['bytes'], 'official explicit argument differs')
            pointer = expected['kind'] in ('storage_address', 'fault_address')
            require(actual['value_kind'] == ('global_buffer' if pointer else 'by_value') and
                    (not pointer or actual['address_space'] == 'global'), 'official argument kind differs')
        seen = set()
        for actual in arguments[len(explicit):]:
            kind = actual['value_kind']
            require(kind in HIDDEN_ARGUMENTS and kind not in seen, 'unknown or duplicate HSA implicit argument')
            offset, size = HIDDEN_ARGUMENTS[kind]
            require(int(actual['offset']) == entry['implicit_argument_offset'] + offset and int(actual['size']) == size,
                    'official implicit argument differs')
            seen.add(kind)
        require({f'hidden_block_count_{axis}' for axis in 'xyz'} <= seen, 'HSA launch guard lacks implicit group counts')


def compile_artifact(source, expected, reflection, profile):
    optimized, obj, image = (source.with_suffix(suffix) for suffix in ('.opt.ll', '.o', '.hsaco'))
    for path in (optimized, obj, image): prepare_output(path)
    def operation(name, arguments, path, content, ceiling):
        status, output, diagnostic = invoke_with_snapshot(path, ceiling, lambda: invoke(profile, name, arguments), content)
        require(status == 0 and not output and not diagnostic, 'official ' + name + ' failed: ' + diagnostic)
    operation('opt', ['-passes=verify,' + profile['optimization_passes'] + ',verify', '-S', str(source), '-o', str(optimized)], source, expected, 33554432)
    optimized_bytes = read_bounded(optimized, 33554432)
    operation('llc', ['-march=amdgcn', '-mcpu=' + reflection['target'], '--amdhsa-code-object-version=5', '-filetype=obj', str(optimized), '-o', str(obj)], optimized, optimized_bytes, 33554432)
    object_bytes = read_bounded(obj, 64 * 1024**2)
    operation('lld', ['-flavor', 'gnu', '--threads=1', '-shared', '--no-undefined', str(obj), '-o', str(image)], obj, object_bytes, 64 * 1024**2)
    image_bytes = read_bounded(image, 64 * 1024**2)
    require(len(image_bytes) >= 64 and image_bytes[:6] == b'\x7fELF\x02\x01' and image_bytes[7:9] == bytes((64, 3)) and
            struct.unpack_from('<HH', image_bytes, 16) == (3, 224), 'invalid HSA v5 AMDGPU ELF code object')
    status, output, diagnostic = invoke_with_snapshot(image, 64 * 1024**2,
        lambda: invoke(profile, 'llvm-readobj', ['--notes', '--elf-output-style=JSON', str(image)], 8 * 1024**2), image_bytes)
    require(status == 0 and not diagnostic, 'official HSA metadata inspection failed')
    check_metadata(json.loads(output), reflection, profile)
    metadata = image.with_suffix('.metadata.json'); prepare_output(metadata); metadata.write_text(output)
    return digest(image_bytes)


def log_artifacts(path):
    pending, records = [], []
    for line in read_bounded(path, 64 * 1024**2).decode().splitlines():
        if line.startswith('{"schema":"seen-amdgpu-artifact-v1"'):
            require(not pending, 'artifact lacks paired metadata'); pending.append(json.loads(line))
        elif line.startswith('{"schema":"seen-gpu-reflection-v1"'):
            require(len(pending) == 1, 'reflection lacks paired artifact'); pending.append(json.loads(line))
        elif line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"'):
            require(len(pending) == 2, 'manifest lacks paired artifact/reflection')
            records.append((*pending, json.loads(line))); pending = []
    require(not pending, 'incomplete AMDGPU artifact record'); return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path); parser.add_argument('names', nargs='+')
    parser.add_argument('--cdna-only', action='append', default=[], metavar='FIXTURE')
    parser.add_argument('--profiles', nargs='+', choices=('fast', 'release', 'undefined', 'address'), default=['fast', 'release', 'undefined', 'address'])
    args = parser.parse_args()
    require(all(re.fullmatch('[A-Za-z0-9_-]{1,64}', name) for name in args.names), 'invalid fixture name')
    require(set(args.cdna_only) <= set(args.names), 'CDNA subset refers to an unknown fixture')
    profile = load_profile(); identity = verify_installation(profile); baseline = None; outputs = []; code_objects = {}
    for mode in args.profiles:
        records = []
        for name in args.names:
            current = log_artifacts(args.root / f'{name}-{mode}.log')
            targets = profile['targets'][:2] if name in args.cdna_only else profile['targets']
            require([item[0]['target'] for item in current] == targets, 'AMDGPU target coverage differs')
            for record, reflection, manifest in current:
                check_artifact(record, reflection, manifest)
                source = args.root / f"{name}-{mode}-{record['target']}.ll"; prepare_output(source)
                raw = record['text'].encode('ascii'); source.write_bytes(raw)
                for path, data in ((source.with_suffix('.json'), reflection), (source.with_suffix('.manifest.json'), manifest)):
                    prepare_output(path); path.write_text(json.dumps(data, separators=(',', ':'), ensure_ascii=False) + '\n')
                image_hash = compile_artifact(source, raw, reflection, profile)
                key = (name, record['target'])
                if key not in code_objects: code_objects[key] = image_hash
                require(code_objects[key] == image_hash, 'HSA code object differs across compiler profiles')
                outputs.append({'fixture': name, 'profile': mode, 'target': record['target'], 'code_object_sha256': image_hash})
            records.extend(current)
        if baseline is None: baseline = records
        require(records == baseline, 'AMDGPU artifact or reflection differs across compiler profiles')
    evidence = {'schema': 'seen-amdgpu-validation-v1', 'classification': 'compile-only', 'official_validation': True,
                'hardware_verified': False, 'gpu_execution_verified': False, 'toolchain': identity, 'profiles': args.profiles,
                'cdna_only_fixtures': args.cdna_only, 'artifacts': outputs}
    (args.root / 'validation.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: AMDGPU official LLVM and HSA code-object ABI hashes geometry storage and profile identity')


if __name__ == '__main__':
    main()
