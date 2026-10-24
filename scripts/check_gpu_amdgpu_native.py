#!/usr/bin/env python3
"""Require real native MFMA instructions for live FP16 and BF16 inputs."""
import argparse
import json
from pathlib import Path
import re

from check_gpu_amdgpu_artifacts import check_artifact, prepare_output
from check_gpu_amdgpu_toolchain import invoke, load_profile, verify_installation
from gpu_certification_io import read_bounded, invoke_with_snapshot
from gpu_launch_contract import digest, require


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('root', type=Path)
    args = parser.parse_args(); profile = load_profile(); identity = verify_installation(profile); records = []
    for target in ('gfx90a', 'gfx942'):
        source = args.root / ('cooperative-fast-' + target + '.ll')
        raw = read_bounded(source, 33554432)
        reflection = json.loads(read_bounded(source.with_suffix('.json'), 4194304))
        manifest = json.loads(read_bounded(source.with_suffix('.manifest.json'), 1048576))
        check_artifact({'text': raw.decode('ascii'), 'target': target}, reflection, manifest)
        optimized = source.with_suffix('.opt.ll'); content = read_bounded(optimized, 33554432)
        assembly = source.with_suffix('.native.s'); prepare_output(assembly)
        status, output, diagnostic = invoke_with_snapshot(optimized, 33554432,
            lambda: invoke(profile, 'llc', ['-march=amdgcn', '-mcpu=' + target, '--amdhsa-code-object-version=5',
                                          '-filetype=asm', str(optimized), '-o', str(assembly)]), content)
        require(status == 0 and not output and not diagnostic, 'native assembly failed: ' + diagnostic)
        data = read_bounded(assembly, 33554432); text = data.decode('ascii')
        for entry, precision in (('mfmaHalf', 'f16'), ('mfmaBfloat', r'bf16(?:_1k)?')):
            function = re.search(r'^' + entry + r':.*?(?=^\.Lfunc_end\d+:)', text, re.M | re.S)
            require(function is not None and re.search(r'\bv_mfma_f32_16x16x16_?' + precision + r'\s', function[0]) is not None,
                    'required native MFMA instruction omitted: ' + entry)
        records.append({'target': target, 'input_sha256': digest(raw), 'assembly_sha256': digest(data), 'precisions': ['fp16', 'bf16']})
    (args.root / 'native-instructions.json').write_text(json.dumps({
        'schema': 'seen-amdgpu-native-instructions-v1', 'classification': 'compile-only',
        'hardware_verified': False, 'gpu_execution_verified': False, 'toolchain': identity, 'artifacts': records,
    }, sort_keys=True) + '\n')
    print('PASS: pinned LLVM emits native FP16/BF16 MFMA instructions for both CDNA targets')


if __name__ == '__main__': main()
