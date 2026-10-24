#!/usr/bin/env python3
"""Digest-pinned LLVM/HSA offline pipeline; never discover or execute a GPU."""
import hashlib
import json
from pathlib import Path
import platform
import re
import sys

import gpu_toolchain_process as process
from gpu_certification_io import read_bounded

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / 'docs/architecture/amdgpu-toolchain.json'
TARGETS = ['gfx90a', 'gfx942', 'gfx1100', 'gfx1101', 'gfx1200']


class ToolchainError(ValueError):
    pass


def require(value, message):
    if not value:
        raise ToolchainError('E_GPU_AMDGPU_TOOLCHAIN: ' + message)


def load_profile():
    document = json.loads(read_bounded(POLICY, 65536))
    require(document['schema'] == 'seen-amdgpu-toolchain-v1', 'unknown policy schema')
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'unsupported toolchain platform')
    profile = document['profiles']['linux-x86_64']
    require(profile['targets'] == TARGETS and profile['optimization_passes'] == 'default<O2>' and
            profile['hsa_code_object_version'] == 5 and profile['maximum_private_bytes_per_thread'] == 65536,
            'unsupported target or compilation policy')
    require(set(profile['tools']) == {'opt', 'llc', 'lld', 'llvm-readobj', 'lli'}, 'incomplete LLVM toolchain')
    for name, tool in profile['tools'].items():
        require(tool['path'] == '/usr/lib/llvm-20/bin/' + name and
                re.fullmatch('[0-9a-f]{64}', tool['sha256']) and len(tool['version_line']) <= 128,
                'invalid LLVM tool identity')
    require(profile['library']['path'] == '/usr/lib/x86_64-linux-gnu/libLLVM.so.20.1' and
            re.fullmatch('[0-9a-f]{64}', profile['library']['sha256']), 'invalid LLVM library identity')
    return profile


def inspect_library(profile):
    library = profile['library']
    path = Path(library['path'])
    require(path.is_file() and not path.is_symlink(), 'LLVM library unavailable')
    before = process.identity(path)
    require(before[2] <= 256 * 1024**2, 'LLVM library exceeds byte ceiling')
    with path.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    require(process.identity(path) == before and digest == library['sha256'], 'LLVM library identity differs')
    return before


def invoke(profile, name, arguments, output_limit=65536):
    require(name in profile['tools'], 'unknown LLVM operation')
    before = inspect_library(profile)
    try:
        result = process.invoke(profile['tools'][name], arguments, output_limit)
    except process.ToolProcessError as error:
        code = 'E_GPU_AMDGPU_TOOLCHAIN_TIMEOUT' if error.timeout else 'E_GPU_AMDGPU_TOOLCHAIN'
        raise ToolchainError(code + ': ' + str(error)) from error
    require(process.identity(Path(profile['library']['path'])) == before, 'LLVM library changed during invocation')
    return result


def verify_installation(profile):
    tools = {}
    for name, tool in profile['tools'].items():
        arguments = ['--version'] if name != 'lld' else ['-flavor', 'gnu', '--version']
        status, output, diagnostic = invoke(profile, name, arguments)
        require(status == 0 and not diagnostic and output.splitlines()[0] == tool['version_line'], 'LLVM version differs')
        tools[name] = {'sha256': tool['sha256'], 'version': tool['version_line']}
    return {'schema': 'seen-amdgpu-toolchain-evidence-v1', 'platform': 'linux-x86_64',
            'classification': 'compile-only', 'tools': tools, 'library_sha256': profile['library']['sha256'],
            'optimization_passes': profile['optimization_passes'], 'hsa_code_object_version': 5,
            'hardware_verified': False, 'gpu_execution_verified': False}


if __name__ == '__main__':
    try:
        print(json.dumps(verify_installation(load_profile()), sort_keys=True))
    except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
