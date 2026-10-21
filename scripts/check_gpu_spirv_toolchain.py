#!/usr/bin/env python3
"""Pinned official SPIR-V tool identity and bounded invocation policy."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import subprocess
import tempfile

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


def memory_budget():
    available = next(int(line.split()[1]) * 1024 for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemAvailable:'))
    maximum = Path('/sys/fs/cgroup/memory.max').read_text().strip()
    if maximum != 'max':
        available = min(available, max(0, int(maximum) - int(Path('/sys/fs/cgroup/memory.current').read_text())))
    budget = min(2 * 1024**3, available // 4)
    require(budget >= 256 * 1024**2, 'insufficient validator memory headroom')
    return budget


def identity(path):
    info = path.stat()
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def inspect_tool(path, expected):
    require(path.is_file() and not path.is_symlink() and os.access(path, os.X_OK), 'official executable is unavailable')
    before = identity(path)
    require(before[2] <= 64 * 1024**2, 'tool binary exceeds byte ceiling')
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    require(identity(path) == before and digest == expected, 'official executable digest differs')
    return before


def invoke(profile, name, arguments, output_limit=65536):
    tool = profile['tools'][name]
    path = Path(tool['path'])
    before = inspect_tool(path, tool['sha256'])
    budget = memory_budget()

    def limits():
        resource.setrlimit(resource.RLIMIT_AS, (budget, budget))
        resource.setrlimit(resource.RLIMIT_CPU, (30, 30))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        resource.setrlimit(resource.RLIMIT_FSIZE, (64 * 1024**2, 64 * 1024**2))

    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            result = subprocess.run([str(path), *arguments], stdout=stdout, stderr=stderr,
                                    timeout=45, env={'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'}, preexec_fn=limits)
        except subprocess.TimeoutExpired as error:
            raise ToolchainError('E_GPU_SPIRV_TOOLCHAIN_TIMEOUT: official tool exceeded its deadline') from error
        require(identity(path) == before, 'official executable changed during invocation')
        require(stdout.tell() <= output_limit and stderr.tell() <= output_limit, 'official tool diagnostic output exceeds ceiling')
        stdout.seek(0); stderr.seek(0)
        output, diagnostic = stdout.read().decode('utf-8', 'replace'), stderr.read().decode('utf-8', 'replace')
    require(result.returncode >= 0, 'official tool terminated by a signal')
    return result.returncode, output, diagnostic


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
