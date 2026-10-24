"""Pinned CPU witness boundary. This cannot validate an Apple Metal artifact."""
import json
from pathlib import Path
import platform

import gpu_toolchain_process as process
from gpu_certification_io import read_bounded, invoke_with_snapshot
from gpu_launch_contract import require, digest


def load_profile():
    root = Path(__file__).resolve().parents[1]
    policy = json.loads(read_bounded(root / 'docs/architecture/metal-toolchain.json', 65536))
    require(policy['schema'] == 'seen-metal-toolchain-v1', 'unknown Metal policy schema')
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'unsupported CPU witness platform')
    profile = policy['profiles']['linux-x86_64']
    require(profile['classification'] == 'diagnostic-cpu' and profile['official_validation'] == 'not-run',
            'CPU witness cannot establish official validation')
    require(profile['clang']['path'] == '/usr/lib/llvm-20/bin/clang', 'unsupported CPU witness compiler path')
    status, output, diagnostic = process.invoke(profile['clang'], ['--version'])
    require(status == 0 and not diagnostic and output.splitlines()[0] == profile['clang']['version_line'],
            'CPU witness compiler version differs')
    return profile


def compile_source(profile, source, executable, headers, expected):
    flags = ['-std=c++17', '-Wno-unknown-attributes', '-O2', '-fno-fast-math', '-ffp-contract=off',
             '-fsanitize=undefined,float-cast-overflow', '-fno-sanitize-recover=all', '-I', str(headers)]
    return invoke_with_snapshot(source, 33554432, lambda: process.invoke(profile['clang'],
        flags + [str(source), '-lstdc++', '-lm', '-o', str(executable)]), expected)


def run_executable(executable):
    raw = read_bounded(executable, 33554432)
    tool = {'path': str(executable.resolve()), 'sha256': digest(raw)}
    return invoke_with_snapshot(executable, 33554432, lambda: process.invoke(tool, []), raw)
