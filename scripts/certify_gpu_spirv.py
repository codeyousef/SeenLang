#!/usr/bin/env python3
"""Certify actual emitted files with pinned tools; never claim GPU execution."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import time

from gpu_certification_io import read_bounded, manifest_from_log, invoke_with_snapshot
from check_gpu_spirv_artifacts import check_layout
from check_gpu_spirv_toolchain import invoke, load_profile, require, verify_installation


def validate(profile, target, path, accepted=True, expected=None):
    status, output, diagnostic = invoke_with_snapshot(
        path, 33554432, lambda: invoke(profile, 'spirv-val', ['--target-env', target, str(path)]), expected)
    require(status == (0 if accepted else 1), 'official validator returned an unexpected status')
    require(not output and (not diagnostic if accepted else bool(diagnostic)), 'official validator diagnostics differ')


def certify(root, profile, seconds=60):
    toolchain = verify_installation(profile)
    records = []
    profile_baseline = None
    for name in ('fast', 'release', 'undefined', 'address'):
        lines = read_bounded(root / f'artifact-{name}.log', 1048576).decode().splitlines()
        manifests = [json.loads(line) for line in lines if line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"')]
        require([item['target'] for item in manifests] == ['vulkan1.2', 'vulkan1.3'], 'profile omitted target manifests')
        if profile_baseline is None:
            profile_baseline = manifests
        require(manifests == profile_baseline, 'artifact manifests differ between execution profiles')
    for target in ('vulkan1.2', 'vulkan1.3'):
        baseline = None
        for copy in ('original', 'relocated'):
            stem = root / f'{copy}-{target}'
            binary = read_bounded(Path(str(stem) + '.spv'), 33554432)
            reflection = read_bounded(Path(str(stem) + '.json'), 4194304)
            manifest = manifest_from_log(Path(str(stem) + '.log'))
            require(manifest['target'] == target and manifest['backend'] == 'spirv' and manifest['byte_order'] == 'little', 'manifest target or encoding differs')
            require(manifest['classification'] == 'compile-only' and manifest['official_validation'] == 'not-run' and
                    manifest['hardware_verified'] is False and manifest['gpu_execution_verified'] is False, 'compiler manifest overclaims validation')
            require(len(binary) >= 20 and len(binary) % 4 == 0 and manifest['byte_count'] == len(binary) and manifest['word_count'] * 4 == len(binary), 'manifest geometry differs')
            require(manifest['sha256'] == hashlib.sha256(binary).hexdigest() and
                    manifest['reflection_sha256'] == hashlib.sha256(reflection).hexdigest(), 'manifest digest differs from actual file')
            current = (binary, reflection, manifest)
            if baseline is None:
                baseline = current
            require(current == baseline, 'relocated source changed deterministic artifacts')
            validate(profile, target, Path(str(stem) + '.spv'), expected=binary)
            words = list(struct.unpack(f'<{len(binary)//4}I', binary))
            check_layout(words, json.loads(reflection))
        stem = root / f'original-{target}'
        assembly, rebuilt = root / f'{target}.spvasm', root / f'{target}-roundtrip.spv'
        status, _, diagnostic = invoke_with_snapshot(
            Path(str(stem) + '.spv'), 33554432,
            lambda: invoke(profile, 'spirv-dis', ['--raw-id', str(Path(str(stem) + '.spv')), '-o', str(assembly)]), binary)
        require(status == 0 and not diagnostic, 'official disassembly failed')
        status, _, diagnostic = invoke_with_snapshot(
            assembly, 33554432,
            lambda: invoke(profile, 'spirv-as', ['--target-env', target, '--preserve-numeric-ids', str(assembly), '-o', str(rebuilt)]))
        require(status == 0 and not diagnostic, 'official assembly failed')
        validate(profile, target, rebuilt)
        roundtrip = read_bounded(rebuilt, 33554432)
        # Assembler owns the generator word; every other header/instruction byte must match.
        require(binary[:8] + binary[12:] == roundtrip[:8] + roundtrip[12:], 'official roundtrip changed instruction bytes')
        records.append({'target': target, 'sha256': manifest['sha256'], 'reflection_sha256': manifest['reflection_sha256'], 'roundtrip': True})
        if target == 'vulkan1.3':
            validate(profile, 'vulkan1.2', Path(str(stem) + '.spv'), False, binary)
    # Guaranteed-invalid inputs exercise the actual SDK, without a local parser rejecting first.
    binary = read_bounded(root / 'original-vulkan1.2.spv', 33554432)
    mutant = root / 'last-malformed.spv'
    cases = [b'', binary[:19], binary[:-1], b'\0\0\0\0' + binary[4:],
             binary[:12] + bytes(4) + binary[16:], binary[:4] + bytes(4) + binary[8:],
             binary + bytes(4), binary + struct.pack('<I', 0x1ffff)]
    for payload in cases:
        mutant.write_bytes(payload); validate(profile, 'vulkan1.2', mutant, False, payload)
    rng = random.Random(1090); deadline = time.monotonic() + seconds; count = 0
    while time.monotonic() < deadline:
        payload = bytearray(binary)
        # Invalid magic/version must reject; the SDK does not check reserved schema.
        if rng.randrange(2):
            struct.pack_into('<I', payload, 0, rng.randrange(0x07230203))
        else:
            struct.pack_into('<I', payload, 4, rng.choice((0, 0xffffffff, 0x7fff0000)))
        mutant.write_bytes(payload); validate(profile, 'vulkan1.2', mutant, False, payload); count += 1
    evidence = {'schema': 'seen-spirv-certification-v1', 'classification': 'compile-only',
                'official_validation': True, 'hardware_verified': False, 'gpu_execution_verified': False,
                'toolchain': toolchain, 'artifacts': records, 'malformed_cases': len(cases),
                'fuzz_seed': 1090, 'fuzz_seconds': seconds, 'fuzz_cases': count}
    (root / 'certification.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: GPU-002C official validation roundtrip reflection hashes relocation and malformed fuzz')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    args = parser.parse_args()
    certify(args.root.resolve(), load_profile())
