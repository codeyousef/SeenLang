#!/usr/bin/env python3
"""Fresh native LLVM20 controls, then exec; a saved receipt never admits ASan.

The admission owner retains the ordinary compiler address budget. Only its
instrumented controls and final fixture receive the finite 64 TiB allowance.
All work retains the same cgroup, private /tmp and caller deadline.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import signal
import select
import struct
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / 'scripts'
CAP = 64 * 1024**4
INSUFFICIENT_CAP = 128 * 1024**2
MAX_LOG = 16 * 1024**2
SHADOW_OFFSET = 0x1000000000
# These are the default Linux/AArch64 constants, not the x86/CUDA layout.
LAYOUT_SOURCE = ('https://github.com/llvm/llvm-project/blob/llvmorg-20.1.2/'
                 'compiler-rt/lib/asan/asan_mapping.h')
ASAN_OPTIONS = 'abort_on_error=1:halt_on_error=1:detect_leaks=1:symbolize=0:protect_shadow_gap=1'


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def bounded_read(path, maximum=MAX_LOG):
    path = Path(path)
    require(path.is_file() and not path.is_symlink(), f'unsafe or missing file: {path}')
    require(path.stat().st_size <= maximum, f'file exceeds evidence bound: {path}')
    return path.read_bytes()


def digest(path):
    path = Path(path)
    require(path.is_file() and path.stat().st_size <= 256 * 1024**2,
            f'identity input missing or oversized: {path}')
    checksum = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            checksum.update(block)
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': checksum.hexdigest()}


def environment():
    result = dict(os.environ)
    for name in ('LD_PRELOAD', 'LD_LIBRARY_PATH', 'LSAN_OPTIONS', 'ASAN_OPTIONS',
                 'ASAN_SYMBOLIZER_PATH', 'UBSAN_OPTIONS', 'TSAN_OPTIONS',
                 'SEEN_FORK_SERIALIZER_TARGET', 'SEEN_FORK_SERIALIZER_ROOT_PID',
                 'CFLAGS', 'CXXFLAGS', 'CPPFLAGS', 'LDFLAGS', 'CPATH', 'C_INCLUDE_PATH',
                 'CPLUS_INCLUDE_PATH', 'LIBRARY_PATH', 'CCC_OVERRIDE_OPTIONS',
                 'CLANG_CONFIG_FILE_SYSTEM_DIR', 'CLANG_CONFIG_FILE_USER_DIR'):
        result.pop(name, None)
    result.update(LC_ALL='C', ASAN_OPTIONS=ASAN_OPTIONS, LSAN_OPTIONS='exitcode=23',
                  UBSAN_OPTIONS='abort_on_error=1:halt_on_error=1:print_stacktrace=0',
                  TSAN_OPTIONS='halt_on_error=1:exitcode=66:report_signal_unsafe=1')
    return result


def child_limit(cap):
    def apply():
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        resource.setrlimit(resource.RLIMIT_AS, (cap, cap))
        soft, hard = resource.getrlimit(resource.RLIMIT_FSIZE)
        size = MAX_LOG if hard == resource.RLIM_INFINITY else min(MAX_LOG, hard)
        resource.setrlimit(resource.RLIMIT_FSIZE, (size, size))
        if resource.getrlimit(resource.RLIMIT_AS) != (cap, cap):
            os._exit(125)
    return apply


def command(argv, proof, name, env, timeout=30, cap=None):
    """Own and drain the command group on failure, deadline, or parent signal."""
    (proof / (name + '.command.json')).write_text(json.dumps([str(x) for x in argv]) + '\n')
    start = time.monotonic()
    require(signal.getsignal(signal.SIGCHLD) == signal.SIG_DFL,
            'native control owner requires unreaped child identity')
    with (proof / (name + '.out')).open('xb') as out, (proof / (name + '.err')).open('xb') as err:
        with subprocess.Popen([str(x) for x in argv], stdout=out, stderr=err, env=env,
                              start_new_session=True,
                              preexec_fn=child_limit(cap) if cap is not None else None) as proc:
            descriptor = None
            try:
                descriptor = os.pidfd_open(proc.pid)
                ready = select.poll()
                ready.register(descriptor, select.POLLIN)
                remaining = max(0, int((start + timeout - time.monotonic()) * 1000))
                if not ready.poll(remaining):
                    raise subprocess.TimeoutExpired(argv, timeout)
                require(os.waitid(os.P_PID, proc.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None,
                        'native command readiness lacked an owned exited leader')
            except BaseException as error:
                (proof / (name + '.status.json')).write_text(json.dumps({
                    'failure': str(error), 'elapsed_seconds': time.monotonic() - start,
                    'timeout_seconds': timeout, 'rlimit_as_bytes': cap}) + '\n')
                raise
            finally:
                # Keep the exited leader unreaped until its group is drained:
                # after wait()/poll() the numeric PGID could belong to someone else.
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                finally:
                    proc.wait()
                    if descriptor is not None:
                        os.close(descriptor)
            status = proc.returncode
    (proof / (name + '.status.json')).write_text(json.dumps({
        'returncode': status, 'elapsed_seconds': time.monotonic() - start,
        'timeout_seconds': timeout, 'rlimit_as_bytes': cap}) + '\n')
    return status, bounded_read(proof / (name + '.out')), bounded_read(proof / (name + '.err'))


def checked(argv, proof, name, env, *, cap=None):
    status, out, err = command(argv, proof, name, env, cap=cap)
    require(status == 0, f'{name} failed ({status}); see {proof}')
    return out.decode('utf-8', errors='strict')


def native_elf(path):
    with Path(path).open('rb') as stream:
        header = stream.read(64)
    require(len(header) == 64 and header[:6] == b'\x7fELF\x02\x01',
            f'not ELF64 little endian: {path}')
    kind, machine = struct.unpack_from('<HH', header, 16)
    require(kind in (2, 3) and machine == 183, f'not native AArch64 executable: {path}')
    return {'type': kind, 'machine': machine, 'class': 64, 'endianness': 'little'}


def package_file(path, package, version, proof, name, env):
    """Bind real bytes to installed authenticated apt package, not a caller hash."""
    actual = Path(path).resolve(strict=True)
    row = checked(['/usr/bin/dpkg-query', '-W', '-f=${Status}\t${Version}\t${Architecture}',
                   package + ':arm64'], proof, name + '-package', env)
    require(row == 'install ok installed\t' + version + '\tarm64',
            f'{package} is not the exact installed native LLVM20 package')
    sums = Path('/var/lib/dpkg/info') / (package + ':arm64.md5sums')
    # Ubuntu may omit the multiarch suffix for non-Multi-Arch packages.
    if not sums.exists():
        sums = Path('/var/lib/dpkg/info') / (package + '.md5sums')
    lines = bounded_read(sums).decode('ascii').splitlines()
    relative = str(actual).lstrip('/')
    entries = [line.split(None, 1)[0] for line in lines
               if len(line.split(None, 1)) == 2 and line.split(None, 1)[1] == relative]
    require(len(entries) == 1, f'{actual} lacks exact package file ownership')
    checksum = hashlib.md5(usedforsecurity=False)
    with actual.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            checksum.update(block)
    require(checksum.hexdigest() == entries[0], f'installed package bytes changed: {actual}')
    return dict(digest(actual), package=package + ':arm64', version=version,
                package_md5=entries[0], package_index=digest(sums))


def snapshot():
    group_text = Path('/proc/self/cgroup').read_text()
    groups = [line[3:] for line in group_text.splitlines() if line.startswith('0::')]
    require(len(groups) == 1 and groups[0].startswith('/') and '..' not in Path(groups[0]).parts,
            'unified owned cgroup unavailable')
    group = Path('/sys/fs/cgroup' + groups[0])
    values = {name: bounded_read(group / name, 65536).decode('ascii').strip()
              for name in ('memory.max', 'memory.swap.max', 'pids.max', 'memory.events', 'pids.events')}
    require(values['memory.max'].isdigit() and 0 < int(values['memory.max'])
            <= int(os.environ['SEEN_MEMORY_GUARD_RSS_KB']) * 1024
            and values['memory.swap.max'] == '0' and values['pids.max'].isdigit()
            and 0 < int(values['pids.max']) <= 24, 'aggregate kernel limits changed')
    root = Path(os.environ['SEEN_ARTIFACT_ROOT']).resolve(strict=True)
    require(root.is_dir() and ROOT in root.parents, 'artifact root escaped checkout')
    require((os.stat('/tmp').st_dev, os.stat('/tmp').st_ino) ==
            (root.stat().st_dev, root.stat().st_ino), 'private /tmp no longer maps to artifacts')
    address_limits = resource.getrlimit(resource.RLIMIT_AS)
    require(0 < address_limits[0] <= int(os.environ['SEEN_MAIN_VMEM_KB']) * 1024 and
            (address_limits[1] == resource.RLIM_INFINITY or address_limits[1] >= CAP),
            'ordinary admission-owner budget or finite-runtime capacity changed')
    require(Path('/proc/sys/kernel/randomize_va_space').read_text().strip() in ('1', '2'),
            'kernel address randomization is disabled')
    return {'cgroup': group_text, 'limits': values,
            'mount_namespace': os.readlink('/proc/self/ns/mnt'),
            'artifact_root': str(root), 'scope_unit': os.environ['SEEN_MEMORY_GUARD_SCOPE_UNIT'],
            'rlimit_as': list(address_limits)}


def verify_scope(proof, name, env):
    if env.get('SEEN_CAPPED_REGRESSION_ACTIVE') == '1':
        argv = ['bash', SCRIPTS / 'run_capped_regression.sh', '--verify-active',
                env['SEEN_CAPPED_REGRESSION_SCOPE'], '--compiler', env['SEEN_CAPPED_REGRESSION_COMPILER']]
    else:
        require(env.get('SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE') == '1', 'capped scope missing')
        argv = ['bash', SCRIPTS / 'run_capped_regression.sh', '--verify-platform-active',
                env['SEEN_CAPPED_PLATFORM_REGRESSION_SCOPE']]
    checked(argv, proof, name + '-binding', env)
    checked(['bash', SCRIPTS / 'run_in_hard_memory_scope.sh', '--label',
             'native ARM ASan controls read-back', '--verify-only', '--'],
            proof, name + '-kernel', env)
    return snapshot()


def validate_control(mode, status, out, err):
    marker = (f'ARM_ASAN_CONTROL soft={CAP} hard={CAP} scale=3 '
              f'offset={SHADOW_OFFSET} aslr_disabled=0\n').encode()
    if mode == 'clean':
        require(status == 0 and out.startswith(marker) and
                out.endswith(b'ARM_ASAN_CONTROL clean-complete\n') and
                b'ERROR: AddressSanitizer' not in err and b'LeakSanitizer:' not in err and
                b'SUMMARY: AddressSanitizer' not in err,
                'native ARM ASan clean control failed under 64 TiB (no admission)')
    elif mode == 'heap':
        require(status in (-signal.SIGABRT, 134) and out == marker and
                b'ERROR: AddressSanitizer: heap-buffer-overflow' in err and
                b'WRITE of size 1' in err and b'SUMMARY: AddressSanitizer: heap-buffer-overflow' in err,
                'native ARM ASan did not diagnose the deliberate one-byte heap overflow')
    else:
        require(status in (-signal.SIGABRT, 134) and b'ARM_ASAN_CONTROL' not in out and
                b'AddressSanitizer' in err and
                b'ReserveShadowMemoryRange failed while trying to map' in err and
                b"Perhaps you're using ulimit -v or ulimit -d" in err,
                'insufficient finite address-space control lacked a real ASan reservation failure')


def report_control_failure(proof, mode):
    """Publish bounded escaped tails; diagnostics cannot admit a control."""
    from collect_verification_logs import read_tail
    require(mode in ('clean', 'heap', 'insufficient'), 'unknown native control')
    for suffix in ('status.json', 'out', 'err'):
        name = mode + '.' + suffix
        try:
            original, content, snapshot = read_tail(ROOT, proof / name, 2048)
            print(f'native ARM ASan {name} tail={len(content)}/{original} '
                  f'truncated={snapshot["truncated"]} '
                  f'changed={snapshot["changed_during_read"]}: {content!r}',
                  file=sys.stderr, flush=True)
        except (OSError, ValueError) as error:
            print(f'native ARM ASan {name} diagnostic unavailable: {error}',
                  file=sys.stderr, flush=True)


def layout_evidence(out, err):
    ranges = {}
    for match in re.finditer(rb'\[0x([0-9a-f]+), 0x([0-9a-f]+)\].*?\|\|\s+(HighMem|HighShadow|ShadowGap|LowShadow|LowMem)', err):
        name = match[3].decode('ascii')
        require(name not in ranges, 'ambiguous runtime shadow layout')
        ranges[name] = [int(match[1], 16), int(match[2], 16)]
    require(set(ranges) == {'HighMem', 'HighShadow', 'ShadowGap', 'LowShadow', 'LowMem'},
            'runtime did not expose its complete actual shadow layout')
    high_end = ranges['HighMem'][1]
    high_start = (high_end >> 3) + SHADOW_OFFSET + 1
    low_end = ((SHADOW_OFFSET - 1) >> 3) + SHADOW_OFFSET
    expected = {'LowMem': [0, SHADOW_OFFSET - 1], 'LowShadow': [SHADOW_OFFSET, low_end],
                'HighMem': [high_start, high_end],
                'HighShadow': [(high_start >> 3) + SHADOW_OFFSET, (high_end >> 3) + SHADOW_OFFSET],
                'ShadowGap': [low_end + 1, (high_start >> 3) + SHADOW_OFFSET - 1]}
    require(ranges == expected, f'runtime layout differs from pinned AArch64 source: {ranges}')
    maps = [(int(a, 16), int(b, 16), perms.decode()) for a, b, perms in
            re.findall(rb'^([0-9a-f]+)-([0-9a-f]+) ([rwxps-]{4}) ', out, re.M)]
    require(maps and sum(end - start for start, end, _ in maps) < CAP,
            'actual runtime mappings exceed finite address allowance')
    for name, permission in (('LowShadow', 'rw-p'), ('HighShadow', 'rw-p'), ('ShadowGap', '---p')):
        start, end = ranges[name]
        cursor = start
        for low, high, perms in sorted(maps):
            if high <= cursor or low > end:
                continue
            require(low <= cursor and perms == permission, f'{name} has missing/wrong kernel mapping')
            cursor = high
            if cursor > end:
                break
        require(cursor > end, f'{name} is not fully kernel mapped')
    return {'source': LAYOUT_SOURCE, 'scale': 3, 'offset': SHADOW_OFFSET,
            'ranges': ranges, 'mapped_bytes': sum(end - start for start, end, _ in maps)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-root', type=Path, required=True)
    parser.add_argument('--compile-log', type=Path, required=True)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    argv = args.command[1:] if args.command[:1] == ['--'] else args.command
    require(argv and platform.system() == 'Linux' and platform.machine() in ('aarch64', 'arm64'),
            'real native Linux ARM64 command required')
    target = Path(argv[0])
    require(args.target_root.is_absolute() and args.target_root.resolve(strict=True) == args.target_root
            and target.is_absolute() and target.resolve(strict=True) == target
            and target.parent == args.target_root and not target.is_symlink(), 'unsafe target path')
    before = snapshot()
    proof = Path(tempfile.mkdtemp(prefix='asan-arm64-admission-', dir=before['artifact_root']))
    print(f'sanitizer-arm64-admission evidence={proof}', file=sys.stderr, flush=True)
    observation = {'schema': 'seen-native-arm64-asan-admission-v1', 'accepted': False,
                   'kernel_before': before, 'target': digest(target),
                   'compile_log': digest(args.compile_log),
                   'helpers': {name: digest(SCRIPTS / name) for name in
                               ('linux_arm64_asan_admission.py', 'linux_arm64_asan_control.c',
                                'run_asan_in_hard_memory_scope.sh')}}
    active_control = None
    try:
        env = environment()
        cap = min(2 * 1024**3, int(env['SEEN_OPT_VMEM_KB']) * 1024)
        verify_scope(proof, 'initial', env)
        observation['package_pin'] = digest(ROOT / 'docs/architecture/amdgpu-toolchain.json')
        pin = json.loads(bounded_read(ROOT / 'docs/architecture/amdgpu-toolchain.json'))
        version = pin['profiles']['linux-x86_64']['package_version']
        require(version == '1:20.1.2-0ubuntu1~24.04.3', 'ARM ASan layout needs review for changed LLVM pin')
        compiler = Path('/usr/bin/clang-20').resolve(strict=True)
        observation['compiler'] = package_file(compiler, 'clang-20', version, proof, 'compiler', env)
        observation['compiler']['elf'] = native_elf(compiler)
        version_text = checked([compiler, '--no-default-config', '--version'],
                               proof, 'compiler-version', env, cap=cap)
        require('clang version 20.1.2' in version_text and 'aarch64' in version_text,
                'compiler version or native target differs')
        resource_dir = Path(checked([compiler, '--no-default-config', '-print-resource-dir'],
                                    proof, 'resource-dir', env, cap=cap).strip()).resolve(strict=True)
        runtime = resource_dir / 'lib/linux/libclang_rt.asan-aarch64.a'
        observation['runtime'] = package_file(runtime, 'libclang-rt-20-dev', version, proof, 'runtime', env)
        runtime_headers = checked(['/usr/bin/readelf', '-hW', runtime],
                                  proof, 'runtime-elf', env)
        machines = re.findall(r'Machine:\s+([^\n]+)', runtime_headers)
        require(machines and all(machine.strip() == 'AArch64' for machine in machines),
                'pinned runtime archive does not contain exclusively AArch64 ELF objects')
        observation['runtime']['elf_machine'] = 'AArch64'
        observation['runtime']['elf_members'] = len(machines)
        observation['target']['elf'] = native_elf(target)
        symbols = checked(['/usr/bin/readelf', '-Ws', target], proof, 'target-symbols', env)
        for symbol in ('__asan_init', '__asan_get_shadow_mapping', '__asan_version_mismatch_check_v8'):
            require(any(row.split()[-1:] == [symbol] and ' UND ' not in row
                        for row in symbols.splitlines()), f'target lacks defined LLVM ASan runtime: {symbol}')
        dynamic = checked(['/usr/bin/readelf', '-dW', target], proof, 'target-dynamic', env)
        require(not re.search(r'\((?:RPATH|RUNPATH)\)|libclang_rt\.asan|libasan\.so', dynamic),
                'unverified dynamic sanitizer/runtime search path')
        comment = checked(['/usr/bin/readelf', '-p', '.comment', target], proof, 'target-compiler', env)
        require('clang version 20.1.2' in comment, 'target lacks pinned native Clang20 object evidence')
        control = proof / 'control'
        compile_argv = [compiler, '--no-default-config', '-std=c11', '-O0', '-g',
                        '-fno-omit-frame-pointer', '-fsanitize=address', '-static-libsan',
                        '-resource-dir', resource_dir, '-fPIE', '-pie', '-Wl,--trace',
                        SCRIPTS / 'linux_arm64_asan_control.c', '-o', control]
        status, out, err = command(compile_argv, proof, 'compile', env, timeout=60, cap=cap)
        require(status == 0 and str(runtime).encode() in out + err,
                'native control compile did not link the exact pinned static ASan archive')
        observation['control'] = dict(digest(control), elf=native_elf(control))
        observation['controls'] = {}
        # Verbosity only observes layout; error, leak and shadow-gap policy is identical.
        control_env = dict(env, ASAN_OPTIONS=ASAN_OPTIONS + ':verbosity=1')
        for mode in ('clean', 'heap', 'insufficient'):
            verify_scope(proof, mode, env)
            limit = INSUFFICIENT_CAP if mode == 'insufficient' else CAP
            active_control = mode
            status, out, err = command([control, 'heap' if mode == 'heap' else 'clean'],
                                       proof, mode, control_env, cap=limit)
            observation['controls'][mode] = {'returncode': status, 'rlimit_as_bytes': limit}
            validate_control(mode, status, out, err)
            if mode == 'clean':
                observation['layout'] = layout_evidence(out, err)
        after = verify_scope(proof, 'final', env)
        require(after == before, 'kernel containment or resource event counters changed during controls')
        require(digest(target)['sha256'] == observation['target']['sha256'], 'target changed during controls')
        require(digest(compiler)['sha256'] == observation['compiler']['sha256'] and
                digest(runtime)['sha256'] == observation['runtime']['sha256'], 'LLVM inputs changed during controls')
        require(all(digest(SCRIPTS / name) == identity
                    for name, identity in observation['helpers'].items()), 'admission source changed during controls')
        resource.setrlimit(resource.RLIMIT_AS, (CAP, CAP))
        target_limits = resource.getrlimit(resource.RLIMIT_AS)
        require(target_limits == (CAP, CAP), 'final fixture finite allowance failed exact read-back')
        observation.update(accepted=True, kernel_after=after, target_rlimit_as=list(target_limits))
        (proof / 'admission.json').write_text(json.dumps(observation, sort_keys=True, indent=2) + '\n')
        print(f'sanitizer-arm64-admission accepted evidence={proof}', file=sys.stderr, flush=True)
        print(f'sanitizer-vmem soft_kb={CAP // 1024} hard_kb={CAP // 1024}', file=sys.stderr, flush=True)
        print('sanitizer-address-layout=default protect_shadow_gap=1', file=sys.stderr, flush=True)
        os.execve(target, argv, env)
    except BaseException as error:
        observation['accepted'] = False
        observation['failure'] = str(error)
        (proof / 'admission.json').write_text(json.dumps(observation, sort_keys=True, indent=2) + '\n')
        if active_control is not None:
            try:
                report_control_failure(proof, active_control)
            except Exception as diagnostic_error:
                print(f'native ARM ASan diagnostic failure: {diagnostic_error}',
                      file=sys.stderr, flush=True)
        raise


def interrupted(signum, _frame):
    raise RuntimeError(f'admission interrupted by signal {signum}')


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    try:
        main()
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        print('RESOURCE STOP: native ARM ASan admission: ' + str(error), file=sys.stderr)
        sys.exit(126)
