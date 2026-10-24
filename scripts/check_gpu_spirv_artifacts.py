#!/usr/bin/env python3
"""Validate retained Seen SPIR-V artifacts, profile identity and reflection ABI."""
import argparse
import json
from pathlib import Path
import struct
from check_gpu_spirv_toolchain import invoke, load_profile, verify_installation


def require(value, message):
    if not value:
        raise ValueError(message)


def instructions(words):
    index = 5
    while index < len(words):
        count, opcode = words[index] >> 16, words[index] & 65535
        require(count > 0 and count <= len(words) - index, 'truncated SPIR-V instruction')
        yield opcode, words[index + 1:index + count]
        index += count


def entry_name(operands):
    encoded = b''.join(struct.pack('<I', word) for word in operands[2:])
    end = encoded.index(b'\0')
    return encoded[:end].decode('utf-8'), 2 + (end + 4) // 4


def check_layout(words, reflection):
    types, variables, decorations, offsets, entries, workgroups, specifications = {}, {}, {}, {}, {}, {}, {}
    for opcode, args in instructions(words):
        if opcode in (20, 21, 22, 23, 29, 30, 32):
            types[args[0]] = (opcode, args[1:])
        elif opcode == 50:
            specifications[args[1]] = (args[0], args[2])
        elif opcode == 59:
            variables[args[1]] = (args[0], args[2])
        elif opcode == 71:
            decorations.setdefault(args[0], {})[args[1]] = args[2:]
        elif opcode == 72 and args[2] == 35:
            offsets[(args[0], args[1])] = args[3]
        elif opcode == 15:
            name, first_interface = entry_name(args)
            require(args[0] == 5 and name not in entries, 'invalid or duplicate compute entry')
            entries[name] = (args[1], args[first_interface:])
        elif opcode == 16 and args[1] == 17:
            workgroups[args[0]] = args[2:]
    require(reflection['hardware_verified'] is False and reflection['gpu_execution_verified'] is False, 'offline artifact claims device verification')
    domains = {item['id']: item for item in reflection['specializations']}
    seen = set()
    for identity, (type_id, default) in specifications.items():
        key = decorations.get(identity, {}).get(1)
        require(key and key[0] in domains and key[0] not in seen, 'specialization identity differs or is duplicated')
        seen.add(key[0])
        domain = domains[key[0]]
        require(default == domain['default'] & 0xffffffff, 'specialization default differs')
        require(types[type_id] == (21, [32, int(domain['type'] == 2)]), 'specialization scalar type differs')
    require(list(entries) == [entry['name'] for entry in reflection['entries']], 'entry/reflection identity differs')
    for entry in reflection['entries']:
        function, interface = entries[entry['name']]
        require(workgroups[function] == entry['workgroup'], 'workgroup/reflection geometry differs')
        bound = {}
        for identity in interface:
            attributes = decorations.get(identity, {})
            if 33 in attributes:
                require(attributes.get(34) == [0], 'descriptor set differs from reflection')
                binding = attributes[33][0]
                require(binding not in bound, 'duplicate entry binding')
                bound[binding] = identity
        expected = {item['binding'] for item in entry['bindings']}
        if entry['fault_binding'] >= 0:
            expected.add(entry['fault_binding'])
        require(set(bound) == expected, 'descriptor binding/reflection identity differs')
        for item in entry['bindings']:
            identity = bound[item['binding']]
            pointer, storage = variables[identity]
            require(storage == (2 if item['kind'] == 'uniform' else 12), 'resource address space differs')
            opcode, pointer_args = types[pointer]
            require(opcode == 32 and pointer_args[0] == storage, 'resource pointer type differs')
            block = pointer_args[1]
            require(2 in decorations.get(block, {}) and offsets.get((block, 0)) == 0, 'resource block layout differs')
            opcode, fields = types[block]
            require(opcode == 30 and len(fields) == 1, 'resource block shape differs')
            value_type = fields[0]
            if storage == 12:
                require(decorations.get(value_type, {}).get(6) == [item['element_stride']], 'storage element stride differs')
                opcode, fields = types[value_type]
                require(opcode == 29 and len(fields) == 1, 'storage runtime array differs')
                value_type = fields[0]
            opcode, value_args = types[value_type]
            if item['lanes'] > 1:
                require(opcode == 23 and value_args[1] == item['lanes'], 'vector lanes differ')
                opcode, value_args = types[value_args[0]]
            scalar = item['scalar_type']
            if scalar == 4:
                require(opcode == 22 and value_args == [32], 'Float32 resource representation differs')
            else:
                require(opcode == 21 and value_args == [32, int(scalar == 2)], 'integer/Boolean resource representation differs')
            attributes = decorations.get(identity, {})
            require((25 in attributes) == (item['access'] == 'write'), 'write-only resource access differs')
            require((24 in attributes) == (storage == 12 and item['access'] == 'read'), 'read-only resource access differs')
        if entry['fault_binding'] >= 0:
            pointer, storage = variables[bound[entry['fault_binding']]]
            require(storage == 12, 'fault binding address space differs')
            block = types[pointer][1][1]
            require(types[block][0] == 30 and len(types[block][1]) == 1 and offsets.get((block, 0)) == 0, 'fault block layout differs')
            require(types[types[block][1][0]] == (21, [32, 0]), 'fault flag representation differs')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('names', nargs='+')
    parser.add_argument('--require-opcodes', default='')
    parser.add_argument('--require-capabilities', default='')
    args = parser.parse_args()
    toolchain = load_profile()
    identity = verify_installation(toolchain)
    baseline = None
    all_opcodes, capabilities = set(), set()
    for profile in ('fast', 'release', 'undefined', 'address'):
        records = []
        for name in args.names:
            path = args.root / f'{name}-{profile}.log'
            require(path.stat().st_size <= 64 * 1024 * 1024, 'fixture log exceeds byte ceiling')
            pending = None
            targets = set()
            for line in path.read_text().splitlines():
                if line.startswith('{"schema":"seen-spirv-'):
                    record = json.loads(line)
                    words = record['words']
                    require(5 <= len(words) <= 8388608, 'artifact word ceiling exceeded')
                    require(all(type(word) is int and 0 <= word <= 0xffffffff for word in words), 'invalid unsigned word')
                    target = record.get('target', 'vulkan1.2')
                    require(target in ('vulkan1.2', 'vulkan1.3') and target not in targets, 'invalid or repeated artifact target')
                    targets.add(target)
                    require(words[:3] == [0x07230203, 0x10600 if target == 'vulkan1.3' else 0x10500, 0], 'artifact header differs')
                    require(1 <= words[3] <= 1048576 and words[4] == 0, 'artifact identity bound differs')
                    binary = args.root / f'{name}-{profile}-{target}.spv'
                    binary.write_bytes(struct.pack(f'<{len(words)}I', *words))
                    status, output, diagnostic = invoke(toolchain, 'spirv-val', ['--target-env', target, str(binary)])
                    require(status == 0 and not output and not diagnostic, 'official validation failed: ' + diagnostic)
                    for opcode, operands in instructions(words):
                        all_opcodes.add(opcode)
                        if opcode == 17:
                            capabilities.add(operands[0])
                    records.append(record)
                    if name != 'primitive':
                        require(pending is None, 'artifact lacks paired reflection')
                        pending = words
                elif line.startswith('{"schema":"seen-gpu-reflection-v1"'):
                    require(pending is not None, 'reflection lacks paired artifact')
                    check_layout(pending, json.loads(line))
                    pending = None
            require(pending is None, 'last artifact lacks paired reflection')
            expected = {'vulkan1.2'} if name == 'primitive' else {'vulkan1.2', 'vulkan1.3'}
            require(targets == expected, 'fixture omitted a required target')
        if baseline is None:
            baseline = records
        require(records == baseline, f'artifacts differ under {profile}')
    for key, values in [('opcodes', all_opcodes), ('capabilities', capabilities)]:
        requested = getattr(args, 'require_' + key)
        required = {int(value) for value in requested.split(',') if value}
        require(required <= values, f'missing required {key}: {sorted(required - values)}')
    evidence = {'schema': 'seen-spirv-validation-v1', 'official_validation': True,
                'device_execution': False, 'toolchain': identity, 'profiles': ['fast', 'release', 'undefined', 'address'],
                'opcodes': sorted(all_opcodes), 'capabilities': sorted(capabilities)}
    (args.root / 'validation.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: Seen-emitted SPIR-V passes official validation, reflection ABI and four-profile identity')


if __name__ == '__main__':
    main()
