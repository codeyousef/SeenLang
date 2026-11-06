"""Run compiler-emitted PTX through the CUDA driver using reflected arguments."""
import argparse
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from check_gpu_ptx_artifacts import check_artifact
from gpu_certification_io import read_bounded


def require(test, message):
    if not test:
        raise RuntimeError(message)


class Driver:
    def __init__(self, required):
        self.lib = C.CDLL('libcuda.so.1')
        pointer, word, size = C.c_void_p, C.c_uint64, C.c_size_t
        signatures = {
            'cuInit': [C.c_uint], 'cuDeviceGetCount': [C.POINTER(C.c_int)],
            'cuDeviceGet': [C.POINTER(C.c_int), C.c_int],
            'cuDeviceGetName': [pointer, C.c_int, C.c_int],
            'cuDeviceGetUuid_v2': [pointer, C.c_int],
            'cuDeviceComputeCapability': [C.POINTER(C.c_int), C.POINTER(C.c_int), C.c_int],
            'cuDriverGetVersion': [C.POINTER(C.c_int)],
            'cuCtxCreate_v2': [C.POINTER(pointer), C.c_uint, C.c_int],
            'cuCtxDestroy_v2': [pointer], 'cuCtxSynchronize': [],
            'cuModuleLoadDataEx': [C.POINTER(pointer), pointer, C.c_uint, pointer, pointer],
            'cuModuleUnload': [pointer],
            'cuModuleGetFunction': [C.POINTER(pointer), pointer, C.c_char_p],
            'cuMemAlloc_v2': [C.POINTER(word), size], 'cuMemFree_v2': [word],
            'cuMemcpyHtoD_v2': [word, pointer, size],
            'cuMemcpyDtoH_v2': [pointer, word, size],
            'cuLaunchKernel': [pointer, C.c_uint, C.c_uint, C.c_uint,
                               C.c_uint, C.c_uint, C.c_uint, C.c_uint,
                               pointer, C.POINTER(pointer), pointer],
        }
        for name, arguments in signatures.items():
            function = getattr(self.lib, name)
            function.argtypes, function.restype = arguments, C.c_int
        status = self.lib.cuInit(0)
        if not required and status == 100:
            raise LookupError('CUDA reports no device')
        require(status == 0, f'cuInit returned CUDA status {status}')
        count = C.c_int()
        self.call('cuDeviceGetCount', C.byref(count))
        if not required and count.value == 0:
            raise LookupError('CUDA reports no device')
        require(count.value > 0, 'no CUDA device')
        self.device = C.c_int()
        self.call('cuDeviceGet', C.byref(self.device), 0)
        name, major, minor, version = C.create_string_buffer(256), C.c_int(), C.c_int(), C.c_int()
        self.call('cuDeviceGetName', name, len(name), self.device)
        self.call('cuDeviceComputeCapability', C.byref(major), C.byref(minor), self.device)
        self.call('cuDriverGetVersion', C.byref(version))
        if not required and (major.value, minor.value) != (8, 9):
            raise LookupError('designated SM89 hardware is unavailable')
        require((major.value, minor.value) == (8, 9), 'designated SM89 hardware is unavailable')
        uuid = C.create_string_buffer(16)
        self.call('cuDeviceGetUuid_v2', uuid, self.device)
        self.identity = {'name': name.value.decode(), 'uuid': uuid.raw.hex(), 'sm': 89, 'driver_api': version.value}
        self.context = C.c_void_p()
        self.call('cuCtxCreate_v2', C.byref(self.context), 0, self.device)

    def call(self, name, *arguments):
        status = getattr(self.lib, name)(*arguments)
        require(status == 0, f'{name} returned CUDA status {status}')

    def close(self):
        self.call('cuCtxDestroy_v2', self.context)


def words(values, signed=False):
    return struct.pack('<' + ('i' if signed else 'I') * len(values), *values)


def launch(driver, module, entry, bindings, expected, blocks=1, expected_fault=0):
    function = C.c_void_p()
    driver.call('cuModuleGetFunction', C.byref(function), module, entry['name'].encode())
    allocations, storage, parameters = [], [], []
    try:
        for payload, length in bindings:
            if length is None:
                require(len(payload) == 16, 'uniform must have its full 16-byte ABI')
                storage.append(C.create_string_buffer(payload, 16))
                parameters.append(C.cast(storage[-1], C.c_void_p))
            else:
                require(0 < len(payload) <= 65536, 'buffer exceeds fixture budget')
                address = C.c_uint64()
                driver.call('cuMemAlloc_v2', C.byref(address), len(payload))
                allocations.append((address, len(payload)))
                host = C.create_string_buffer(payload, len(payload))
                driver.call('cuMemcpyHtoD_v2', address, host, len(payload))
                storage.extend((address, C.c_uint32(length)))
                parameters.extend(C.cast(C.byref(value), C.c_void_p) for value in storage[-2:])
        fault = None
        if entry['requires_fault_buffer']:
            fault = C.c_uint64()
            driver.call('cuMemAlloc_v2', C.byref(fault), 4)
            allocations.append((fault, 4))
            zero = C.c_uint32()
            driver.call('cuMemcpyHtoD_v2', fault, C.byref(zero), 4)
            storage.append(fault)
            parameters.append(C.cast(C.byref(fault), C.c_void_p))
        require(len(parameters) == entry['parameter_count'], 'physical argument count differs')
        storage_count = len(allocations) - (1 if fault is not None else 0)
        require(all(type(index) is int and 0 <= index < storage_count and
                    len(wanted) == allocations[index][1] for index, wanted in expected.items()),
                'fixture oracle does not match its physical storage buffers')
        arguments = (C.c_void_p * len(parameters))(*parameters)
        driver.call('cuLaunchKernel', function, blocks, 1, 1, *entry['workgroup'], 0, None, arguments, None)
        driver.call('cuCtxSynchronize')
        outputs = []
        for address, size in allocations:
            output = C.create_string_buffer(size)
            driver.call('cuMemcpyDtoH_v2', output, address, size)
            outputs.append(output.raw)
        for index, wanted in expected.items():
            require(outputs[index] == wanted, f"{entry['name']} buffer {index} differs: {outputs[index].hex()} != {wanted.hex()}")
        if fault is not None:
            require(struct.unpack('<I', outputs[-1])[0] == expected_fault, 'device fault word differs')
    finally:
        for address, _ in reversed(allocations):
            driver.call('cuMemFree_v2', address)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('--kind', required=True, choices=['layout', 'capabilities', 'packed', 'cooperative', 'bf16'])
    parser.add_argument('--require-device', action='store_true')
    parser.add_argument('--compiler', required=True, type=Path)
    parser.add_argument('--cli-artifact', action='store_true',
                        help='validate one direct CLI emission, not four host fixture profiles')
    args = parser.parse_args()
    cgroup = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().strip().split('::', 1)[1].lstrip('/')
    controls = {name: (cgroup / name).read_text().strip() for name in ('memory.max', 'memory.swap.max', 'memory.oom.group', 'pids.max')}
    require(controls['memory.swap.max'] == '0' and controls['memory.oom.group'] == '1' and 0 < int(controls['memory.max']) < 64 * 1024**3 and 0 < int(controls['pids.max']) <= 24, 'invalid aggregate containment')
    artifact_root = Path(os.environ['SEEN_ARTIFACT_ROOT'])
    require(artifact_root.is_absolute() and artifact_root.is_dir() and not artifact_root.is_symlink(), 'invalid artifact root')
    # Hardware JIT output must not populate a user-wide ~/.nv cache.
    os.environ['CUDA_CACHE_PATH'] = str(artifact_root / 'cuda-driver-cache')
    os.environ['CUDA_CACHE_DISABLE'] = '1'
    try:
        driver = Driver(args.require_device)
    except (LookupError, OSError) as error:
        if args.require_device:
            raise
        if isinstance(error, OSError) and not str(error).startswith('libcuda.so.1: cannot open shared object file: No such file or directory'):
            raise
        print(json.dumps({'schema': 'seen-ptx-hardware-evidence-v1', 'gpu_execution_verified': False, 'classification': 'unsupported', 'reason': str(error)}, sort_keys=True))
        return
    records = []
    try:
        profiles = ('compiler-cli',) if args.cli_artifact else ('fast', 'release', 'undefined', 'address')
        for profile in profiles:
            if args.cli_artifact:
                manifests = [json.loads(line) for line in
                             read_bounded(args.root / f'{args.kind}-emit.log', 65536).decode().splitlines()
                             if line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"')]
                require(len(manifests) == 1, 'expected one CLI artifact manifest')
                manifest = manifests[0]
                text = read_bounded(args.root / f'{args.kind}.ptx', 33554432).decode('ascii')
                reflection = json.loads(read_bounded(args.root / f'{args.kind}.reflection.json', 1048576))
                record = {'schema': 'seen-ptx-artifact-v1', 'target': manifest['target'], 'text': text}
                check_artifact(record, reflection, manifest)
                require(record['target'] == 'sm_89', 'CLI artifact is not SM89')
                checks = run_checks(driver, args.kind, text, reflection)
                records.append({'profile': profile, 'ptx_sha256': hashlib.sha256(text.encode()).hexdigest(), 'checks': checks})
                continue
            log = read_bounded(args.root / f'{args.kind}-{profile}.log', 67108864).decode('utf-8')
            pending, selected = [], []
            for line in log.splitlines():
                if line.startswith('{"schema":"seen-ptx-artifact-v1"'):
                    require(not pending, 'unpaired artifact')
                    pending.append(json.loads(line))
                elif line.startswith('{"schema":"seen-gpu-reflection-v1"'):
                    require(len(pending) == 1, 'unpaired reflection')
                    pending.append(json.loads(line))
                elif line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"'):
                    require(len(pending) == 2, 'unpaired manifest')
                    record, reflection = pending
                    pending = []
                    check_artifact(record, reflection, json.loads(line))
                    if record['target'] == 'sm_89':
                        selected.append((record['text'], reflection))
            require(not pending and len(selected) == 1, 'expected one validated SM89 artifact')
            text, reflection = selected[0]
            checks = run_checks(driver, args.kind, text, reflection)
            records.append({'profile': profile, 'ptx_sha256': hashlib.sha256(text.encode()).hexdigest(), 'checks': checks})
    finally:
        driver.close()
    controls.update({name: (cgroup / name).read_text().strip()
                     for name in ('memory.peak', 'pids.peak', 'memory.events', 'pids.events')})
    require(args.compiler.is_file() and not args.compiler.is_symlink(), 'invalid compiler identity')
    with args.compiler.open('rb') as compiler:
        compiler_hash = hashlib.file_digest(compiler, 'sha256').hexdigest()
    tested_entries = {
        'layout': ['integers', 'vectors'],
        'capabilities': ['atomic_memory', 'groups', 'float_groups', 'sequential'],
        'packed': ['packed_words'],
        'cooperative': ['matrix_half', 'matrix_shared'],
        'bf16': ['matrix_bfloat', 'matrix_shared'],
    }[args.kind]
    print(json.dumps({'schema': 'seen-ptx-hardware-evidence-v1', 'device': driver.identity,
                      'kind': args.kind, 'tested_entries': tested_entries, 'profiles': records,
                      'source_commit': subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip(),
                      'compiler_sha256': compiler_hash,
                      'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                      'classification': 'experimental-hardware',
                      'gpu_execution_verified': True, 'fallback_count': 0, 'containment': controls}, sort_keys=True))


def run_checks(driver, kind, text, reflection):
    entries = {entry['name']: entry for entry in reflection['entries']}
    module = C.c_void_p()
    checks = 0
    try:
        source = C.create_string_buffer(text.encode())
        driver.call('cuModuleLoadDataEx', C.byref(module), source, 0, None, None)
        if kind == 'layout':
            for count in (1, 31, 32, 63, 64, 65, 129):
                values = [(-i if i % 2 else i) for i in range(count)]
                expected = [(-v if v < 0 else v + 7 if v < 4 else v + 11) for v in values]
                sentinel = 0x12345678
                launch(driver, module, entries['integers'], [(words(values, True), count), (words([sentinel] * (count + 1)), count), (words([count, 0, 0, 0]), None)], {0: words(values, True), 1: words(expected + [sentinel], True)}, (count + 63) // 64)
                checks += count + 1
            launch(driver, module, entries['integers'], [(words([1]), 0), (words([0, 0x12345678]), 1), (words([1, 0, 0, 0]), None)], {1: words([7, 0x12345678])}, expected_fault=1)
            checks += 3
            for enabled in (0, 1, 2):
                original = struct.pack('<4f', 9, 8, 7, 6)
                launch(driver, module, entries['vectors'], [(struct.pack('<4f', 1, 2, 3, 0), 1), (original, 1), (struct.pack('<4f', 4, 5, 6, 0), None), (words([enabled, 0, 0, 0]), None)], {0: struct.pack('<4f', 1, 2, 3, 0), 1: struct.pack('<4f', 5, 7, 9, 6) if enabled else original})
                checks += 4
        elif kind == 'capabilities':
            launch(driver, module, entries['atomic_memory'], [(words([1] * 64), 64), (words([0] * 64), 64)], {0: words([7] * 64), 1: words([1] * 64)})
            launch(driver, module, entries['groups'], [(words([0] * 64), 64)], {0: words([32] * 32 + [33] * 32)})
            inputs = struct.pack('<' + 'f' * 192, *([1, 2, 3, 0] * 48))
            expected = struct.pack('<' + 'f' * 192, *([32, 64, 96, 0] * 32 + [16, 32, 48, 0] * 16))
            launch(driver, module, entries['float_groups'], [(inputs, 48), (bytes(len(inputs)), 48)], {1: expected})
            launch(driver, module, entries['sequential'], [(words([0]), 1), (words([0]), 1)], {0: words([4]), 1: words([1])})
            checks = 128 + 64 + 144 + 2
        elif kind == 'packed':
            expected = [0] * 29
            expected[:13] = [0x3fc00000, 0x3e00, 0x3fc0, 0xfd, 3, 0xd, 3, 3, 3, 0x3c, 3, 0xfffffffd, 3]
            launch(driver, module, entries['packed_words'],
                   [(words([0, 0, 0, 0]), None), (words([0, 0, 0, 0]), None),
                    (struct.pack('<4f', 1.5, 0, 0, 0), None), (words([127, 0, 0, 0]), None),
                    (bytes(29 * 4), 29), (bytes(28 * 4), 28), (bytes(27 * 4), 27)],
                   {0: words(expected), 1: bytes(28 * 4), 2: bytes(27 * 4)})
            checks = 84
        else:
            # Asymmetric, exactly representable matrices distinguish both
            # major orders, every row/column, and two independent warps.
            a = [[row % 3 + k % 2 - 1 for k in range(16)] for row in range(16)]
            b = [[column % 5 - k % 3 for column in range(16)] for k in range(16)]
            def encode(values):
                if kind == 'cooperative':
                    return struct.pack('<' + 'e' * len(values), *values)
                return struct.pack('<' + 'H' * len(values), *[
                    struct.unpack('<I', struct.pack('<f', value))[0] >> 16 for value in values])
            left = encode([a[row][k] for row in range(16) for k in range(16)])
            right = encode([b[k][column] for column in range(16) for k in range(16)])
            reference = [sum(a[row][k] * b[k][column] for k in range(16))
                         for row in range(16) for column in range(16)]
            expected = struct.pack('<' + 'f' * 512, *(reference * 2))
            entry = entries['matrix_half' if kind == 'cooperative' else 'matrix_bfloat']
            launch(driver, module, entry, [(left, 128), (right, 128), (bytes(2048), 512)], {0: left, 1: right, 2: expected})
            launch(driver, module, entries['matrix_shared'], [(bytes(1024), 256)],
                   {0: struct.pack('<' + 'f' * 256, *([1] * 256))})
            checks = 768
        driver.call('cuModuleUnload', module)
        module = C.c_void_p()
    finally:
        if module.value:
            driver.call('cuModuleUnload', module)
    return checks


if __name__ == '__main__':
    main()
