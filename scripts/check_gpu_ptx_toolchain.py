#!/usr/bin/env python3
"""Pinned offline NVIDIA assembler identity; no driver or device is opened."""
import json
import os
from pathlib import Path
import platform
import re
import gpu_toolchain_process as tool_process

POLICY = Path(__file__).resolve().parents[1] / 'docs/architecture/ptx-toolchain.json'


class ToolchainError(ValueError):
    pass


def require(value, message):
    if not value:
        raise ToolchainError('E_GPU_PTX_TOOLCHAIN: ' + message)


def pinned_profile():
    require(POLICY.is_file() and not POLICY.is_symlink() and POLICY.stat().st_size <= 65536, 'invalid policy file')
    policy = json.loads(POLICY.read_text())
    require(policy['schema'] == 'seen-ptx-toolchain-v1', 'unknown policy schema')
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'unsupported toolchain platform')
    profile = policy['profiles']['linux-x86_64']
    require(set(profile['tools']) == {'ptxas'}, 'incomplete official toolchain')
    require(re.fullmatch('[0-9a-f]{64}', profile['tools']['ptxas']['sha256']), 'invalid tool identity')
    require(re.fullmatch('[0-9a-f]{64}', profile['package_sha256']), 'invalid package identity')
    require(profile['package_url'].startswith('https://files.pythonhosted.org/packages/'), 'invalid package origin')
    require(profile['package_member'] == 'nvidia/cuda_nvcc/bin/ptxas', 'invalid package member')
    require(profile['targets'] == ['sm_70', 'sm_75', 'sm_80', 'sm_86', 'sm_89', 'sm_90', 'sm_100'], 'target profile differs')
    return profile


def load_profile():
    profile = pinned_profile()
    path = Path(os.environ.get('SEEN_PTXAS', ''))
    require(path.is_absolute(), 'SEEN_PTXAS must select an absolute pinned assembler path')
    return {**profile, 'tools': {'ptxas': {**profile['tools']['ptxas'], 'path': str(path)}}}


def invoke(profile, name, arguments, output_limit=65536):
    require(name in profile['tools'], 'unknown official tool')
    try:
        return tool_process.invoke(profile['tools'][name], arguments, output_limit)
    except tool_process.ToolProcessError as error:
        code = 'E_GPU_PTX_TOOLCHAIN_TIMEOUT' if error.timeout else 'E_GPU_PTX_TOOLCHAIN'
        raise ToolchainError(code + ': ' + str(error)) from error


def verify_installation(profile=None):
    profile = profile or load_profile()
    status, output, diagnostic = invoke(profile, 'ptxas', ['--version'], 16384)
    require(status == 0 and not diagnostic and profile['version_line'] in output.splitlines(), 'official tool version differs')
    return {'schema': 'seen-ptx-toolchain-evidence-v1', 'platform': 'linux-x86_64',
            'package': profile['package'], 'package_version': profile['package_version'],
            'package_sha256': profile['package_sha256'], 'targets': profile['targets'],
            'tools': {'ptxas': {'sha256': profile['tools']['ptxas']['sha256'], 'version': output.strip()}},
            'hardware_verified': False, 'gpu_execution_verified': False}


if __name__ == '__main__':
    try:
        print(json.dumps(verify_installation(), sort_keys=True))
    except (ToolchainError, OSError, KeyError, ValueError) as error:
        raise SystemExit(str(error))
