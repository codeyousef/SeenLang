#!/usr/bin/env python3
"""Pinned official SPIR-V tool identity and bounded invocation policy."""
import json
from pathlib import Path
import platform
import re
import gpu_toolchain_process as tool_process

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / 'docs/architecture/spirv-toolchain.json'


class ToolchainError(ValueError):
    pass


def require(value, message):
    if not value:
        raise ToolchainError('E_GPU_SPIRV_TOOLCHAIN: ' + message)


def load_profile():
    require(POLICY.is_file() and not POLICY.is_symlink() and POLICY.stat().st_size <= 65536, 'invalid policy file')
    policy = json.loads(POLICY.read_text())
    require(policy['schema'] == 'seen-spirv-toolchain-v1', 'unknown policy schema')
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'unsupported toolchain platform')
    profile = policy['profiles']['linux-x86_64']
    require(set(profile['tools']) == {'spirv-val', 'spirv-as', 'spirv-dis'}, 'incomplete official toolchain')
    for name, tool in profile['tools'].items():
        require(tool['path'] == '/usr/bin/' + name and re.fullmatch('[0-9a-f]{64}', tool['sha256']), 'invalid tool identity')
    return profile


def process_policy(operation, *arguments):
    try:
        return operation(*arguments)
    except tool_process.ToolProcessError as error:
        code = 'E_GPU_SPIRV_TOOLCHAIN_TIMEOUT' if error.timeout else 'E_GPU_SPIRV_TOOLCHAIN'
        raise ToolchainError(code + ': ' + str(error)) from error


def memory_budget():
    return process_policy(tool_process.memory_budget)


identity = tool_process.identity


def inspect_tool(path, expected):
    return process_policy(tool_process.inspect_tool, path, expected)


def invoke(profile, name, arguments, output_limit=65536):
    return process_policy(tool_process.invoke, profile['tools'][name], arguments, output_limit)


def verify_installation(profile=None):
    profile = profile or load_profile()
    records = {}
    for name in profile['tools']:
        status, output, diagnostic = invoke(profile, name, ['--version'], 16384)
        require(status == 0 and not diagnostic and output.startswith(profile['version_prefix']), 'official tool version differs')
        if name == 'spirv-val':
            require('under Vulkan 1.2 semantics' in output and 'under Vulkan 1.3 semantics' in output, 'required Vulkan targets are missing')
        records[name] = {'sha256': profile['tools'][name]['sha256'], 'version': output.strip()}
    return {'schema': 'seen-spirv-toolchain-evidence-v1', 'platform': 'linux-x86_64',
            'package': profile['package'], 'package_version': profile['package_version'],
            'package_sha256': profile['package_sha256'], 'tools': records}


if __name__ == '__main__':
    try:
        print(json.dumps(verify_installation(), sort_keys=True))
    except (ToolchainError, OSError, KeyError, ValueError) as error:
        raise SystemExit(str(error))
