#!/usr/bin/env python3
"""Certify deterministic Seen MSL and, only with approved Apple tools, AIR/metallib.

The official tools compile only: this entrypoint never creates a Metal device or
executes a GPU. SDK binary bytes are observed, not promised path-independent.

The two-stage AIR/metallib producer contract is Apple's Metal Tools Listing 8-1:
https://developer.apple.com/library/archive/documentation/Miscellaneous/Conceptual/MetalProgrammingGuide/Dev-Technique/Dev-Technique.html
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import random
import struct
import sys
import time

from check_gpu_metal_artifacts import check_artifact
from check_gpu_metal_toolchain import (LANGUAGES, TARGETS, ToolchainUnavailable,
                                       invoke, load_profile, verify_installation)
from gpu_certification_io import (invoke_with_snapshot, manifest_from_log,
                                  prepare_output, read_bounded, require)
from gpu_certification_profiles import profile_manifests

SOURCE_LIMIT = 32 * 1024**2
IMAGE_LIMIT = 32 * 1024**2
PROFILES = ('fast', 'release', 'undefined', 'address')


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def write_json(path, value):
    prepare_output(path)
    path.write_text(json.dumps(value, sort_keys=True) + '\n', encoding='utf-8')


def profile_manifest(root, target):
    lines = read_bounded(root / 'artifact-fast.log', 1048576).decode('utf-8').splitlines()
    documents = [json.loads(line) for line in lines
                 if line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"')]
    matches = [value for value in documents if value['target'] == target]
    require(len(matches) == 1, 'missing unique four-profile target manifest')
    return matches[0]


def reject_integrity_mutations(record, reflection, manifest):
    """Change exact integrity snapshots, not the semantic corpus owned by gate A."""
    mutations = (
        ('source-byte', lambda r, f, m: r.update(text=r['text'] + '\n')),
        ('source-hash', lambda r, f, m: m.update(sha256='0' * 64)),
        ('source-length', lambda r, f, m: m.update(byte_count=m['byte_count'] + 1)),
        ('reflection-byte', lambda r, f, m: f.update(module=f['module'] + '-changed')),
        ('reflection-hash', lambda r, f, m: m.update(reflection_sha256='0' * 64)),
        ('target', lambda r, f, m: r.update(target='unknown')),
        ('backend', lambda r, f, m: m.update(backend='cuda')),
        ('official-claim', lambda r, f, m: m.update(official_validation='passed')),
        ('execution-claim', lambda r, f, m: f.update(gpu_execution_verified=True)),
    )
    rejected = []
    for name, mutate in mutations:
        changed = copy.deepcopy((record, reflection, manifest))
        mutate(*changed)
        try:
            check_artifact(*changed)
        except ValueError:
            rejected.append(name)
        else:
            raise ValueError('Metal integrity mutation accepted: ' + name)
    return rejected


def certify_cpu(root):
    profile_manifests(root, TARGETS, 'metal')
    profile_records = []
    for profile in PROFILES:
        for phase in ('compile', 'run'):
            status = read_bounded(root / f'artifact-{profile}.{phase}.status', 16)
            require(status == b'0\n', 'integrity fixture did not complete ' + profile + ' ' + phase)
        log = read_bounded(root / f'artifact-{profile}.log', 1048576)
        profile_records.append({'profile': profile, 'compile_status': 0, 'run_status': 0,
                                'log_sha256': digest(log), 'ownership_baseline_verified': True})
    seen_source = read_bounded(root / 'original-source.seen', 1048576)
    require(seen_source == read_bounded(root / 'relocated/source.seen', 1048576),
            'relocated Seen source differs from its original snapshot')
    records = []
    for target in TARGETS:
        baseline = None
        mutations = None
        for location in ('original', 'relocated'):
            stem = root / f'{location}-{target}'
            raw = read_bounded(stem.with_suffix('.metal'), SOURCE_LIMIT)
            reflection_bytes = read_bounded(stem.with_suffix('.json'), 4194304)
            manifest = manifest_from_log(stem.with_suffix('.log'))
            require(digest(reflection_bytes) == manifest['reflection_sha256'],
                    'reflection file bytes differ from the emitted integrity snapshot')
            reflection = json.loads(reflection_bytes)
            require(manifest['schema'] == 'seen-gpu-artifact-manifest-v1' and
                    reflection['schema'] == 'seen-gpu-reflection-v1' and
                    reflection['ir_schema'] == 'seen-gpu-ir-v1' and
                    reflection['module'] == 'gpu-artifact.seen',
                    'unexpected Metal integrity/reflection schema or stable module identity')
            record = {'target': target, 'text': raw.decode('ascii')}
            check_artifact(record, reflection, manifest)
            require(manifest == profile_manifest(root, target),
                    'CLI artifact differs from the four-profile integrity fixture')
            current = (raw, reflection_bytes, manifest)
            if baseline is None:
                baseline = current
                mutations = reject_integrity_mutations(record, reflection, manifest)
            require(current == baseline, 'relocation changed MSL, reflection or integrity manifest')
            write_json(stem.with_suffix('.manifest.json'), manifest)
        records.append({'target': target, 'msl_version': LANGUAGES[target],
                        'sha256': manifest['sha256'],
                        'reflection_sha256': manifest['reflection_sha256'],
                        'reflection_file_sha256': digest(reflection_bytes),
                        'manifest_file_sha256': digest(read_bounded(stem.with_suffix('.manifest.json'), 1048576)),
                        'source_relocation_verified': True,
                        'four_profile_hash_identity_verified': True,
                        'rejected_integrity_mutations': mutations})
    return {'status': 'passed', 'classification': 'native-cpu-integrity-only',
            'seen_source_sha256': digest(seen_source), 'stable_module_id': 'gpu-artifact.seen',
            'profiles': profile_records, 'artifacts': records,
            'deterministic_scope': 'MSL bytes, reflection bytes and integrity manifests',
            'official_validation': 'not-run', 'gpu_execution_verified': False}


def check_air(image):
    # LLVM documents raw BC C0 DE and the little-endian 0B17C0DE wrapper:
    # https://llvm.org/docs/BitCodeFormat.html#bitcode-wrapper-format
    require(len(image) >= 8, 'official compiler emitted a truncated AIR object')
    if image[:4] == b'\xde\xc0\x17\x0b':
        require(len(image) >= 20, 'truncated AIR bitcode wrapper')
        _, version, offset, size, _ = struct.unpack_from('<5I', image)
        require(version == 0 and offset >= 20 and size >= 8 and offset + size <= len(image),
                'invalid AIR bitcode wrapper extent')
        require(image[offset:offset + 4] == b'BC\xc0\xde', 'AIR wrapper omitted LLVM bitcode')
        return 'llvm-bitcode-wrapper'
    require(image[:4] == b'BC\xc0\xde', 'official compiler did not emit LLVM AIR bitcode')
    return 'llvm-bitcode'


def check_metallib(image, entries):
    # Limited format checks, not an invented Apple semantic validator. Observed
    # header layout: https://github.com/YuAo/MetalLibraryArchive#header
    require(len(image) >= 88 and image[:4] == b'MTLB', 'invalid Metal library signature/header')
    require(struct.unpack_from('<Q', image, 16)[0] == len(image), 'Metal library file size differs')
    for field in (24, 40, 56, 72):
        offset, size = struct.unpack_from('<QQ', image, field)
        require(offset <= len(image) and size <= len(image) - offset,
                'Metal library section lies outside its file')
    require(all(('seen_metal_entry_' + entry['name']).encode('ascii') + b'\0' in image
                for entry in entries), 'Metal library omitted a reflected kernel name')


def native_call(root, profile, tool, args, source, expected):
    status, stdout, stderr = invoke_with_snapshot(
        source, max(SOURCE_LIMIT, IMAGE_LIMIT),
        lambda: invoke(profile, tool, args, output_limit=65536), expected)
    # A finite number of calls, each with capped diagnostics. Keep real producer
    # status and diagnostics, including warnings, rather than normalize them.
    with (root / 'sdk-invocations.jsonl').open('a', encoding='utf-8') as stream:
        stream.write(json.dumps({'tool': tool, 'arguments': args, 'status': status,
                                 'stdout': stdout, 'stderr': stderr,
                                 'input_sha256': digest(expected)}, sort_keys=True) + '\n')
    return status, stdout, stderr


def compile_arguments(profile, target, source, output):
    return ['-std=' + LANGUAGES[target], '-mmacosx-version-min=' + target[5:] + '.0',
            '-isysroot', profile['sdk']['path'], '-c', str(source), '-o', str(output)]


def compile_library(root, profile, target, source, expected):
    raw = read_bounded(source, SOURCE_LIMIT)
    require(digest(raw) == expected['sha256'], 'official source differs from CPU-validated bytes')
    reflection_bytes = read_bounded(source.with_suffix('.json'), 4194304)
    require(digest(reflection_bytes) == expected['reflection_file_sha256'],
            'official reflection differs from CPU-validated bytes')
    reflection = json.loads(reflection_bytes)
    air = source.with_suffix('.air')
    library = source.with_suffix('.metallib')
    prepare_output(air)
    prepare_output(library)
    status, _, _ = native_call(root, profile, 'metal', compile_arguments(profile, target, source, air), source, raw)
    require(status == 0, 'official Metal compiler failed for ' + source.name)
    image = read_bounded(air, IMAGE_LIMIT)
    air_format = check_air(image)
    status, _, _ = native_call(root, profile, 'metallib', [str(air), '-o', str(library)], air, image)
    require(status == 0, 'official Metal library producer failed for ' + source.name)
    linked = read_bounded(library, IMAGE_LIMIT)
    check_metallib(linked, reflection['entries'])
    return {'source': source.name, 'source_sha256': digest(raw),
            'air': air.name, 'air_bytes': len(image), 'air_sha256': digest(image), 'air_format': air_format,
            'metallib': library.name, 'metallib_bytes': len(linked), 'metallib_sha256': digest(linked),
            'metal_status': 0, 'metallib_status': 0,
            'metallib_header_and_kernel_names_verified': True}


def reject_source(root, profile, target, payload, marker):
    source = root / 'last-malformed.metal'
    output = root / 'last-malformed.air'
    prepare_output(source)
    prepare_output(output)
    source.write_bytes(payload)
    status, stdout, stderr = native_call(root, profile, 'metal',
                                        compile_arguments(profile, target, source, output), source, payload)
    require(status == 1 and marker in stderr and 'error:' in stderr,
            'official syntax rejection was not a normal compiler error with its expected diagnostic')
    # Partial producer outputs are not accepted as successful AIR artifacts.
    if output.exists() or output.is_symlink():
        read_bounded(output, IMAGE_LIMIT)
        prepare_output(output)
    return {'target': target, 'status': status, 'marker': marker,
            'source_sha256': digest(payload), 'diagnostic_sha256': digest(stderr.encode('utf-8'))}


def certify_official(root, profile, evidence):
    prepare_output(root / 'sdk-invocations.jsonl')
    records = []
    evidence['official_artifacts'] = records
    expected_by_target = {record['target']: record for record in evidence['cpu']['artifacts']}
    for target in TARGETS:
        pair = [compile_library(root, profile, target, root / f'{location}-{target}.metal', expected_by_target[target])
                for location in ('original', 'relocated')]
        records.append({'target': target, 'outputs': pair,
                        'observed_air_bytes_equal': pair[0]['air_sha256'] == pair[1]['air_sha256'],
                        'observed_metallib_bytes_equal': pair[0]['metallib_sha256'] == pair[1]['metallib_sha256']})
    rejected = []
    evidence['official_rejections'] = rejected
    # Empty translation units are legal MSL. A preprocessing #error is an
    # unconditional negative witness; separate malformed declarations check syntax.
    for target in TARGETS:
        raw = read_bounded(root / f'original-{target}.metal', SOURCE_LIMIT)
        require(digest(raw) == expected_by_target[target]['sha256'],
                'rejection baseline differs from CPU-validated bytes')
        for prefix, suffix, marker in (
                (b'#error SEEN_METAL_REQUIRED_REJECTION\n', b'', 'SEEN_METAL_REQUIRED_REJECTION'),
                (b'', b'\nkernel void seen_invalid_declaration( {\n', 'seen_invalid_declaration'),
                (b'', b'\nseen_invalid_type seen_invalid_symbol;\n', 'seen_invalid_type')):
            rejected.append(reject_source(root, profile, target, prefix + raw + suffix, marker))
    rng = random.Random(1099)
    started = time.monotonic()
    deadline = started + 60
    count = 0
    while count < 128 and time.monotonic() < deadline:
        target = TARGETS[count % len(TARGETS)]
        raw = read_bounded(root / f'original-{target}.metal', SOURCE_LIMIT)
        require(digest(raw) == expected_by_target[target]['sha256'],
                'rejection baseline differs from CPU-validated bytes')
        marker = 'SEEN_METAL_FUZZ_' + str(rng.randrange(1, 1 << 30))
        payload = ('#error ' + marker + '\n').encode('ascii') + raw
        rejected.append(reject_source(root, profile, target, payload, marker))
        count += 1
    require(count >= 32, 'official rejection fuzz did not make bounded minimum progress')
    evidence['fuzz'] = {'seed': 1099, 'cases': count, 'seconds': time.monotonic() - started,
                        'max_seconds_between_invocations': 60, 'max_cases': 128}
    evidence['external_binary_determinism'] = 'not-required; actual per-invocation bytes and equality recorded'
    evidence['official_validation'] = 'passed'
    evidence['status'] = 'passed'


def certify(root, policy_path=None):
    require(root.is_absolute() and root.is_dir() and not root.is_symlink(), 'unsafe certification directory')
    evidence = {'schema': 'seen-metal-certification-v1', 'classification': 'compile-only',
                'status': 'failed', 'cpu': {'status': 'not-run'}, 'official_validation': 'not-run',
                'hardware_verified': False, 'gpu_execution_verified': False,
                'artifact_directory': root.name,
                'policy': str(policy_path) if policy_path is not None else None}
    destination = root / 'certification.json'
    try:
        evidence['cpu'] = certify_cpu(root)
        write_json(destination, evidence)
        try:
            profile = load_profile(policy_path)
            evidence['toolchain'] = verify_installation(profile)
        except ToolchainUnavailable as error:
            evidence['status'] = 'unsupported'
            evidence['unsupported_reason'] = str(error)
            write_json(destination, evidence)
            print('UNSUPPORTED: Metal official SDK not run; native CPU integrity passed: ' + str(error), file=sys.stderr)
            return 126
        evidence['official_validation'] = 'failed'
        certify_official(root, profile, evidence)
    except (OSError, ValueError, KeyError, TypeError, IndexError, UnicodeError, struct.error) as error:
        evidence['status'] = 'failed'
        if evidence['cpu']['status'] == 'not-run':
            evidence['cpu']['status'] = 'failed'
        evidence['error'] = str(error)
        write_json(destination, evidence)
        print('metal-certification: ' + str(error), file=sys.stderr)
        return 1
    write_json(destination, evidence)
    print('PASS: Metal MSL integrity four-profile ownership relocation official AIR/metallib and rejection fuzz; no GPU execution')
    return 0


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--policy', type=Path)
    arguments = parser.parse_args()
    raise SystemExit(certify(arguments.root, arguments.policy))
