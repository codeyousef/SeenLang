"""Independent physical-argument and integrity checks for GPU text artifacts."""
import hashlib
import json
import math


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def physical_parameters(entry, abi):
    geometry = entry['workgroup']
    require(len(geometry) == 3 and all(type(v) is int and 1 <= v <= 1024 for v in geometry) and
            math.prod(geometry) <= 1024, 'invalid workgroup geometry')
    require(len(entry['bindings']) <= 64, 'logical parameter ceiling exceeded')
    parameters, offset = [], 0

    def add(binding, kind, size, alignment):
        nonlocal offset
        offset = (offset + alignment - 1) // alignment * alignment
        parameters.append({'index': len(parameters), 'binding': binding, 'kind': kind,
                           'offset': offset, 'bytes': size, 'alignment': alignment})
        offset += size

    for index, binding in enumerate(entry['bindings']):
        require(binding['binding'] == index and binding['group'] == 0, 'logical binding order differs')
        if binding['kind'] == 'uniform':
            add(index, 'uniform_value', 16, 16)
        else:
            require(binding['kind'] == 'storage', 'unknown resource kind')
            add(index, 'storage_address', 8, 8)
            add(index, 'storage_length', 4, 4)
    expected_fault = len(entry['bindings']) if entry['requires_fault_buffer'] else -1
    require(entry['fault_binding'] == expected_fault, 'fault binding differs')
    if entry['requires_fault_buffer']:
        add(expected_fault, 'fault_address', 8, 8)
    require(entry['parameter_abi'] == abi and entry['parameters'] == parameters and
            entry['parameter_count'] == len(parameters) and entry['packed_parameter_bytes'] == offset <= 4096,
            'reflected physical launch plan differs')
    return parameters, offset


def check_abi_digest(entry, specializations, extra_keys=(), *, packed=True):
    keys = ('workgroup', 'bindings', 'required_capabilities', 'shared_bytes', 'requires_fault_buffer',
            'fault_binding', 'parameter_abi')
    if packed: keys += ('packed_parameter_bytes', 'parameters', 'parameter_count')
    keys += extra_keys
    abi = {key: entry[key] for key in keys}
    abi['specializations'] = specializations
    canonical = json.dumps(abi, separators=(',', ':'), ensure_ascii=False).encode()
    require(digest(canonical) == entry['abi_sha256'], 'entry ABI digest differs')


def check_text_manifest(record, reflection, manifest, backend, abi):
    target, text = record['target'], record['text']
    require(reflection['backend'] == manifest['backend'] == backend and
            reflection['target'] == manifest['target'] == target, 'backend/target identity differs')
    require(manifest['classification'] == 'compile-only' and manifest['official_validation'] == 'not-run', 'invalid artifact classification')
    require(manifest['byte_order'] == 'utf-8' and manifest['word_count'] == 0 and manifest['parameter_abi'] == abi,
            'invalid text artifact encoding or ABI')
    for document in (reflection, manifest):
        require(document['hardware_verified'] is False and document['gpu_execution_verified'] is False, 'offline artifact claims execution')
    raw = text.encode('ascii')
    require(all(byte in (9, 10) or 32 <= byte <= 126 for byte in raw), 'invalid text artifact byte')
    require(manifest['byte_count'] == len(raw) <= 33554432 and manifest['sha256'] == digest(raw), 'text artifact byte hash differs')
    canonical = json.dumps(reflection, separators=(',', ':'), ensure_ascii=False).encode()
    require(manifest['reflection_sha256'] == digest(canonical), 'reflection hash differs')
