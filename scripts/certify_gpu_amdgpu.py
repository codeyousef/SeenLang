#!/usr/bin/env python3
"""Certify HIP files with pinned LLVM/HSA tools without executing a GPU."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import time

from check_gpu_amdgpu_artifacts import check_artifact, compile_artifact, prepare_output
from check_gpu_amdgpu_toolchain import invoke, load_profile, verify_installation
from gpu_certification_io import (read_bounded, manifest_from_log, invoke_with_snapshot,
                                  require)
from gpu_certification_profiles import profile_manifests


def reject_ir(profile, path, payload):
    prepare_output(path)
    path.write_bytes(payload)
    status, output, diagnostic = invoke_with_snapshot(
        path, 33554432,
        lambda: invoke(profile, 'opt', ['-passes=verify', '-disable-output', str(path)]), payload)
    require(status == 1 and not output and 'error:' in diagnostic and
            ('expected ' in diagnostic or "value doesn't match function result type" in diagnostic),
            'official LLVM rejection was not a parser/type diagnostic')


def malformed_cases(text):
    require('ret void' in text and 'target triple = "amdgcn-amd-amdhsa"' in text,
            'invalid LLVM mutation baseline')
    return ('define amdgpu_kernel void @unfinished(',
            text.replace('ret void', 'seen_invalid_opcode\n  ret void', 1),
            text.replace('ret void', 'ret i32 1', 1),
            text.replace('target triple = "amdgcn-amd-amdhsa"', 'target triple = 1', 1),
            text + '\nseen_invalid_top_level\n',
            text.replace('\n}\n', '\n', 1))


def certify(root, profile, seconds=60):
    require(root.is_dir() and not root.is_symlink(), 'unsafe certification directory')
    toolchain = verify_installation(profile)
    profile_manifests(root, profile['targets'], 'hip')
    records = []
    for target in profile['targets']:
        baseline = None
        image_hash = None
        for copy in ('original', 'relocated'):
            stem = root / f'{copy}-{target}'
            source = stem.with_suffix('.ll')
            raw = read_bounded(source, 33554432)
            reflection_bytes = read_bounded(stem.with_suffix('.json'), 4194304)
            reflection = json.loads(reflection_bytes)
            manifest = manifest_from_log(stem.with_suffix('.log'))
            check_artifact({'target': target, 'text': raw.decode('ascii')}, reflection, manifest)
            current = (raw, reflection_bytes, manifest)
            if baseline is None:
                baseline = current
            require(current == baseline, 'relocated source changed LLVM, reflection or integrity metadata')
            compiled = compile_artifact(source, raw, reflection, profile)
            if image_hash is None:
                image_hash = compiled
            require(compiled == image_hash, 'relocated source changed the HSA code object')
        records.append({'target': target, 'sha256': manifest['sha256'],
                        'reflection_sha256': hashlib.sha256(reflection_bytes).hexdigest(),
                        'code_object_sha256': image_hash, 'source_relocation_verified': True,
                        'hsa_v5_verified': True})
    text = read_bounded(root / 'original-gfx90a.ll', 33554432).decode('ascii')
    mutant = root / 'last-malformed.ll'
    cases = malformed_cases(text)
    for payload in cases:
        require(payload != text, 'malformed case did not change its baseline')
        reject_ir(profile, mutant, payload.encode('ascii'))
    rng = random.Random(1096)
    deadline = time.monotonic() + seconds
    count = 0
    while time.monotonic() < deadline:
        mode = rng.randrange(3)
        if mode == 0:
            payload = text.replace('ret void', f'seen_invalid_{rng.randrange(1, 1 << 30)}\n  ret void', 1)
        elif mode == 1:
            payload = text.replace('ret void', f'ret i32 {rng.randrange(1, 1 << 30)}', 1)
        else:
            payload = text.replace('target triple = "amdgcn-amd-amdhsa"',
                                   f'target triple = {rng.randrange(1, 1 << 30)}', 1)
        require(payload != text, 'fuzz did not change its LLVM baseline')
        reject_ir(profile, mutant, payload.encode('ascii'))
        count += 1
    require(count >= 32 and time.monotonic() >= deadline, 'official rejection fuzz omitted duration or progress')
    evidence = {'schema': 'seen-amdgpu-certification-v1', 'classification': 'compile-only',
                'official_validation': True, 'hardware_verified': False, 'gpu_execution_verified': False,
                'toolchain': toolchain, 'artifacts': records, 'profile_identity': True,
                'malformed_cases': len(cases), 'fuzz_seed': 1096, 'fuzz_seconds': seconds, 'fuzz_cases': count}
    destination = root / 'certification.json'
    prepare_output(destination)
    destination.write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: HIP AMDGPU official LLVM HSA metadata hashes relocation and malformed fuzz')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    arguments = parser.parse_args()
    try:
        certify(arguments.root, load_profile())
    except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
        raise SystemExit('amdgpu-certification: ' + str(error)) from error
