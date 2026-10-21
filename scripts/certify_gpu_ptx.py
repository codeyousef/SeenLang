#!/usr/bin/env python3
"""Certify emitted PTX files and rejection with the pinned offline assembler."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import time

from check_gpu_ptx_artifacts import check_artifact
from check_gpu_ptx_toolchain import invoke, load_profile, require, verify_installation
from gpu_certification_io import read_bounded, manifest_from_log, invoke_with_snapshot


def assemble(profile, target, source, accepted=True, expected=None):
    output = source.with_suffix('.cubin')
    if output.exists() or output.is_symlink():
        require(output.is_file() and not output.is_symlink(), 'unsafe assembler destination')
        output.unlink()
    status, stdout, diagnostic = invoke_with_snapshot(
        source, 33554432,
        lambda: invoke(profile, 'ptxas', ['-arch=' + target, str(source), '-o', str(output)]), expected)
    require(status == (0 if accepted else 255) and not stdout,
            'official assembler returned an unexpected status or stdout')
    if accepted:
        require(not diagnostic, 'official assembler produced diagnostics')
        image = read_bounded(output, 64 * 1024**2)
        require(len(image) >= 64 and image[:6] == b'\x7fELF\x02\x01' and
                struct.unpack_from('<H', image, 18)[0] == 190, 'invalid CUDA ELF code object')
    else:
        require(bool(diagnostic), 'official rejection omitted diagnostics')
        if output.exists() or output.is_symlink():
            require(output.is_file() and not output.is_symlink(), 'unsafe partial assembler output')
            output.unlink()


def profile_manifests(root, targets):
    baseline = None
    for profile in ('fast', 'release', 'undefined', 'address'):
        lines = read_bounded(root / f'artifact-{profile}.log', 1048576).decode().splitlines()
        manifests = [json.loads(line) for line in lines
                     if line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"')]
        require([item['target'] for item in manifests] == targets, 'profile omitted target manifests')
        if baseline is None:
            baseline = manifests
        require(manifests == baseline, 'artifact integrity manifests differ between build profiles')


def malformed_cases(text):
    require(text.startswith('.version 8.0\n') and 'ret;' in text, 'invalid mutation baseline')
    return ('', text[:18], text.replace('.version 8.0', '.version 99.0', 1),
            text.replace('.address_size 64', '.address_size 7', 1),
            text.replace('.target sm_80', '.target sm_999', 1),
            text.replace('ret;', 'seen_invalid_opcode;\nret;', 1),
            text.replace('.reg .b32', '.reg .b7', 1), text + '\n.invalid_directive;\n')


def certify(root, profile, seconds=60):
    toolchain = verify_installation(profile)
    profile_manifests(root, profile['targets'])
    records = []
    for target in profile['targets']:
        baseline = None
        for copy in ('original', 'relocated'):
            stem = root / f'{copy}-{target}'
            source = stem.with_suffix('.ptx')
            text = read_bounded(source, 33554432).decode('ascii')
            reflection = read_bounded(stem.with_suffix('.json'), 4194304)
            manifest = manifest_from_log(stem.with_suffix('.log'))
            check_artifact({'target': target, 'text': text}, json.loads(reflection), manifest)
            current = (text, reflection, manifest)
            if baseline is None:
                baseline = current
            require(current == baseline, 'relocated source changed PTX, reflection or integrity metadata')
            assemble(profile, target, source, expected=text.encode('ascii'))
        records.append({'target': target, 'sha256': manifest['sha256'],
                        'reflection_sha256': hashlib.sha256(reflection).hexdigest(),
                        'source_relocation_verified': True, 'cuda_elf_verified': True})
    text = read_bounded(root / 'original-sm_80.ptx', 33554432).decode('ascii')
    mutant = root / 'last-malformed.ptx'
    cases = malformed_cases(text)
    for payload in cases:
        mutant.write_bytes(payload.encode('ascii'))
        assemble(profile, 'sm_80', mutant, False, payload.encode('ascii'))
    rng = random.Random(1093)
    start = time.monotonic()
    deadline = start + seconds
    count = 0
    while time.monotonic() < deadline:
        mode = rng.randrange(3)
        if mode == 0:
            payload = text.replace('.version 8.0', f'.version {rng.randrange(90, 1000)}.0', 1)
        elif mode == 1:
            payload = text.replace('ret;', f'seen_invalid_{rng.randrange(1, 1 << 30)};\nret;', 1)
        else:
            payload = text.replace('.reg .b32', f'.reg .b{rng.choice((0, 7, 31, 63, 100))}', 1)
        require(payload != text, 'mutation did not change its baseline')
        mutant.write_bytes(payload.encode('ascii'))
        assemble(profile, 'sm_80', mutant, False, payload.encode('ascii'))
        count += 1
    require(count >= 32 and time.monotonic() >= deadline, 'official rejection fuzz omitted duration or progress')
    evidence = {'schema': 'seen-ptx-certification-v1', 'classification': 'compile-only',
                'official_validation': True, 'hardware_verified': False, 'gpu_execution_verified': False,
                'toolchain': toolchain, 'artifacts': records, 'profile_identity': True,
                'malformed_cases': len(cases), 'fuzz_seed': 1093, 'fuzz_seconds': seconds, 'fuzz_cases': count}
    (root / 'certification.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: CUDA PTX official assembly CUDA ELF reflection hashes relocation and malformed fuzz')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    arguments = parser.parse_args()
    certify(arguments.root.resolve(), load_profile())
