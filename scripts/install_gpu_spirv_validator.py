#!/usr/bin/env python3
"""Link the pinned official static validator into its runtime-loadable C ABI."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import tempfile

from check_gpu_spirv_toolchain import invoke, load_profile, require, verify_installation

ROOT = Path(__file__).resolve().parent.parent
SCOPE = 'seen-spirv-validator-sdk'
MAX_ARCHIVE_BYTES = 8 * 1024**2
MAX_IMAGE_BYTES = 8 * 1024**2
SMOKE_SOURCE = b'''OpCapability Shader
OpMemoryModel Logical GLSL450
OpEntryPoint GLCompute %main "main"
OpExecutionMode %main LocalSize 1 1 1
%void = OpTypeVoid
%function = OpTypeFunction %void
%main = OpFunction %void None %function
%entry = OpLabel
OpReturn
OpFunctionEnd
'''


def validate_library(path, binary):
    """Exercise the real vendor C ABI, including diagnostic/context destruction."""
    library = ctypes.CDLL(str(path))
    library.spvParseTargetEnv.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_int)]
    library.spvParseTargetEnv.restype = ctypes.c_bool
    library.spvContextCreate.argtypes = [ctypes.c_int]
    library.spvContextCreate.restype = ctypes.c_void_p
    library.spvContextDestroy.argtypes = [ctypes.c_void_p]
    library.spvContextDestroy.restype = None
    library.spvValidateBinary.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32),
                                        ctypes.c_size_t, ctypes.POINTER(ctypes.c_void_p)]
    library.spvValidateBinary.restype = ctypes.c_int32
    library.spvDiagnosticDestroy.argtypes = [ctypes.c_void_p]
    library.spvDiagnosticDestroy.restype = None
    environment = ctypes.c_int()
    require(library.spvParseTargetEnv(b'vulkan1.2', ctypes.byref(environment)),
            'official validator cannot select Vulkan 1.2')
    words = struct.unpack('<' + 'I' * (len(binary) // 4), binary)
    outcomes = []
    for malformed in (False, True):
        data = (ctypes.c_uint32 * len(words))(*words)
        if malformed:
            data[0] = 0xdeadbeef
        context = library.spvContextCreate(environment.value)
        require(bool(context), 'official validator context allocation failed')
        diagnostic = ctypes.c_void_p()
        try:
            outcomes.append(library.spvValidateBinary(context, data, len(words),
                                                       ctypes.byref(diagnostic)))
        finally:
            library.spvDiagnosticDestroy(diagnostic)
            library.spvContextDestroy(context)
    require(outcomes == [0, -4], 'official validator accepted malformed bytes or rejected valid SPIR-V')
    return outcomes


def install(destination, archive=None):
    profile = load_profile()
    runtime = profile['runtime_validator']
    require(destination.is_absolute() and destination.parent.is_dir(),
            'installation needs an absolute existing parent')
    require(destination.parent.resolve() == destination.parent and not destination.is_symlink(),
            'validator destination must be canonical and not a symlink')
    require(destination.name == runtime['library_name'], 'validator library filename differs')
    require(not destination.exists() or destination.is_file(), 'validator destination is not a regular file')
    archive = archive if archive is not None else Path(runtime['archive_path'])
    descriptor = os.open(archive, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        info = os.fstat(descriptor)
        require(stat.S_ISREG(info.st_mode) and 0 < info.st_size <= MAX_ARCHIVE_BYTES,
                'validator archive is not a bounded regular file')
        with os.fdopen(descriptor, 'rb', closefd=False) as source:
            data = source.read(MAX_ARCHIVE_BYTES + 1)
    finally:
        os.close(descriptor)
    require(len(data) <= MAX_ARCHIVE_BYTES and hashlib.sha256(data).hexdigest() == runtime['archive_sha256'],
            'validator archive digest or size differs')
    # No link/build starts until the inherited kernel scope and toolchain pass.
    subprocess.run(['bash', str(ROOT / 'scripts/run_capped_regression.sh'),
                    '--verify-platform-active', SCOPE], check=True, timeout=30)
    verify_installation(profile)
    with tempfile.TemporaryDirectory(dir=destination.parent, prefix='.spirv-validator-') as temporary:
        work = Path(temporary)
        snapshot = work / 'vendor.a'
        snapshot.write_bytes(data)
        image = work / runtime['library_name']
        command = ['clang++', '-fuse-ld=lld', '-shared', '-Wl,--threads=1', '-Wl,-z,defs',
                   '-Wl,-soname,' + runtime['library_name'], '-Wl,--whole-archive',
                   str(snapshot), '-Wl,--no-whole-archive', '-o', str(image)]
        # GNU timeout owns a process group, including clang's linker child.
        subprocess.run(['timeout', '--kill-after=5s', '120s', *command], check=True)
        require(image.is_file() and 0 < image.stat().st_size <= MAX_IMAGE_BYTES,
                'linked validator image exceeds its bound')
        assembly = work / 'smoke.spvasm'
        assembly.write_bytes(SMOKE_SOURCE)
        module = work / 'smoke.spv'
        status, output, diagnostic = invoke(profile, 'spirv-as',
                                             ['--target-env', 'vulkan1.2', str(assembly), '-o', str(module)])
        require(status == 0 and not output and not diagnostic, 'official assembler smoke failed')
        binary = module.read_bytes()
        require(20 <= len(binary) <= 4096 and len(binary) % 4 == 0,
                'official assembler emitted an invalid smoke module size')
        outcomes = validate_library(image, binary)
        image_hash = hashlib.sha256(image.read_bytes()).hexdigest()
        image_bytes = image.stat().st_size
        image.chmod(0o755)
        os.replace(image, destination)
    return {'schema': 'seen-spirv-runtime-validator-v1', 'package_version': profile['package_version'],
            'archive_sha256': runtime['archive_sha256'], 'library': str(destination),
            'library_sha256': image_hash, 'library_bytes': image_bytes,
            'valid_and_malformed_statuses': outcomes, 'gpu_execution_verified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--archive', type=Path, help='exact pinned archive; defaults to the Ubuntu package path')
    arguments = parser.parse_args()
    try:
        print(json.dumps(install(arguments.destination, arguments.archive), sort_keys=True))
    except (OSError, ValueError, KeyError, AttributeError, subprocess.SubprocessError) as error:
        raise SystemExit('spirv-validator-install: ' + str(error))
