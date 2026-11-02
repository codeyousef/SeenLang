#!/usr/bin/env python3
"""Explicitly approved, digest-pinned Apple SDK tools; offline compilation only.

Policy is supplied by the operator, never discovered or provisioned here. Sources:
https://developer.apple.com/documentation/metal/building-a-shader-library-by-precompiling-source-files
https://developer.apple.com/library/archive/documentation/Miscellaneous/Conceptual/MetalProgrammingGuide/Dev-Technique/Dev-Technique.html
https://developer.apple.com/metal/Metal-Shading-Language-Specification.pdf
https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vm/vm_unix.c
https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/proc_info.h
https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.h
https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.c
https://github.com/llvm/llvm-project/blob/main/clang/include/clang/Options/Options.td

No SDK or executable hashes are shipped as a purported official installation.
"""
import argparse
import ctypes
import errno
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import plistlib
import re
import selectors
import signal
import stat
import subprocess
import sys
import tempfile
import time
from xml.parsers.expat import ExpatError

from gpu_certification_io import read_bounded

TARGETS = ['macos13', 'macos14', 'macos15']
LANGUAGES = dict(zip(TARGETS, ['metal3.0', 'metal3.1', 'metal3.2']))
SCHEMA = 'seen-metal-official-toolchain-v1'
MIB = 1024**2


class ToolchainError(ValueError):
    pass


class ToolchainUnavailable(ToolchainError):
    pass


class _SDKTerminated(BaseException):
    """Unwind owned SDK resources before redispatching the caller's SIGTERM."""


def require(value, message):
    if not value:
        raise ToolchainError('E_GPU_METAL_TOOLCHAIN: ' + message)


def _unavailable(message):
    raise ToolchainUnavailable('E_GPU_METAL_TOOLCHAIN_UNSUPPORTED: ' + message)


def _object(value, keys, label):
    require(type(value) is dict and set(value) == set(keys), 'invalid ' + label + ' fields')


def _text(value, ceiling=4096):
    return (type(value) is str and 0 < len(value) <= ceiling and
            all(ord(char) >= 32 and ord(char) != 127 for char in value))


def _path(value):
    require(_text(value) and value.startswith('/') and
            str(Path(value)) == value and '..' not in Path(value).parts,
            'expected an absolute canonical installation path')
    return Path(value)


def _digest(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None,
            'invalid SHA-256 identity')


def _validate(profile):
    _object(profile, ('schema', 'platform', 'targets', 'languages', 'sdk', 'tools'), 'policy')
    require(profile['schema'] == SCHEMA, 'unknown official policy schema')
    _object(profile['platform'], ('system', 'machine'), 'platform')
    require(profile['platform']['system'] == 'Darwin' and
            profile['platform']['machine'] in ('arm64', 'x86_64'), 'invalid approved platform')
    require(type(profile['targets']) is list and profile['targets'] == TARGETS,
            'unsupported target order')
    _object(profile['languages'], TARGETS, 'language')
    require(profile['languages'] == LANGUAGES, 'unsupported Metal language versions')
    _object(profile['sdk'], ('path', 'settings_sha256'), 'SDK')
    _path(profile['sdk']['path'])
    _digest(profile['sdk']['settings_sha256'])
    _object(profile['tools'], ('metal', 'metallib'), 'tool')
    for name, tool in profile['tools'].items():
        _object(tool, ('path', 'sha256', 'version_line') if name == 'metal' else
                ('path', 'sha256'), name)
        require(_path(tool['path']).name == name, 'unexpected official tool filename')
        _digest(tool['sha256'])
        if name == 'metal':
            require(_text(tool['version_line'], 512), 'invalid approved metal version line')


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate policy key')
        result[key] = value
    return result


def _constant(value):
    raise ToolchainError('E_GPU_METAL_TOOLCHAIN: nonfinite policy number: ' + value)


def _float(value):
    result = float(value)
    require(math.isfinite(result), 'nonfinite policy number')
    return result


def _identity(path):
    info = path.lstat()
    return (info.st_dev, info.st_ino, info.st_mode, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


def _pinned(path, expected, ceiling, executable=False):
    require(path.resolve(strict=True) == path, 'installation path is not resolved')
    before = _identity(path)
    require(stat.S_ISREG(before[2]) and (not executable or os.access(path, os.X_OK)),
            'approved installation file is unavailable')
    content = read_bounded(path, ceiling)
    require(hashlib.sha256(content).hexdigest() == expected and _identity(path) == before,
            'approved installation file identity differs')
    return before, content


def _installation(profile):
    """Check identities before host selection as bad supplied policies are errors."""
    sdk = _path(profile['sdk']['path'])
    require(sdk.resolve(strict=True) == sdk and sdk.is_dir(), 'SDK root is not a canonical directory')
    identities = {'sdk_root': _identity(sdk)}
    identity, content = _pinned(sdk / 'SDKSettings.plist', profile['sdk']['settings_sha256'], MIB)
    identities['sdk_settings'] = identity
    settings = plistlib.loads(content)
    require(type(settings) is dict, 'invalid SDK settings plist')
    version, canonical = settings.get('Version'), settings.get('CanonicalName')
    require(_text(version, 64) and re.fullmatch(r'[0-9]+(?:\.[0-9]+){0,2}', version) is not None and
            int(version.split('.')[0]) >= 15 and _text(canonical, 128) and
            canonical == 'macosx' + version, 'SDK does not identify a macOS 15-or-newer SDK')
    metadata = {'path': str(sdk), 'settings_sha256': profile['sdk']['settings_sha256'],
                'version': version, 'canonical_name': canonical}
    if _text(settings.get('ProductBuildVersion'), 128):
        metadata['product_build_version'] = settings['ProductBuildVersion']
    for name, tool in profile['tools'].items():
        identities[name], _ = _pinned(Path(tool['path']), tool['sha256'], 256 * MIB, True)
    return identities, metadata


def _host(profile):
    if platform.system() != 'Darwin' or platform.machine() not in ('arm64', 'x86_64'):
        _unavailable('official Apple SDK validation requires Darwin arm64 or x86_64; no execution')
    require(profile['platform'] == {'system': platform.system(), 'machine': platform.machine()},
            'approved platform differs from the current host')


def load_profile(policy_path: Path | None = None):
    if policy_path is None:
        _unavailable('no explicitly approved official Apple SDK policy; no execution')
    try:
        document = json.loads(read_bounded(Path(policy_path), 65536),
                              object_pairs_hook=_pairs, parse_constant=_constant, parse_float=_float)
        _validate(document)
        _installation(document)
        _host(document)
        return document
    except ToolchainError:
        raise
    except (OSError, ValueError, TypeError, OverflowError, RuntimeError, ExpatError) as error:
        raise ToolchainError('E_GPU_METAL_TOOLCHAIN: invalid supplied policy or installation: ' +
                             str(error)) from error


class _TaskInfo(ctypes.Structure):
    # proc_taskinfo, XNU bsd/sys/proc_info.h; PROC_PIDTASKINFO = 4.
    _fields_ = [(name, ctypes.c_uint64) for name in
                ('virtual', 'resident', 'user', 'system', 'threads_user', 'threads_system')] + [
                    ('counters', ctypes.c_int32 * 12)]


def _native():
    system = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
    system.sysctlbyname.argtypes = [ctypes.c_char_p, ctypes.c_void_p,
                                   ctypes.POINTER(ctypes.c_size_t), ctypes.c_void_p, ctypes.c_size_t]
    system.sysctlbyname.restype = ctypes.c_int
    proc = ctypes.CDLL('/usr/lib/libproc.dylib', use_errno=True)
    proc.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64,
                                 ctypes.c_void_p, ctypes.c_int]
    proc.proc_pidinfo.restype = ctypes.c_int
    proc.proc_listpgrppids.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
    proc.proc_listpgrppids.restype = ctypes.c_int
    return system, proc


def _sysctl(system, name):
    value = ctypes.c_uint32()
    size = ctypes.c_size_t(ctypes.sizeof(value))
    require(system.sysctlbyname(name.encode('ascii'), ctypes.byref(value), ctypes.byref(size),
                               None, 0) == 0 and size.value == ctypes.sizeof(value),
            'cannot obtain current Darwin memory accounting: ' + name)
    return value.value


def _memory_budget(system):
    # These two disjoint XNU queues exclude active, inactive, and compressed pages.
    # Never derive a budget from installed RAM or double-count speculative pages.
    pages = _sysctl(system, 'vm.page_free_count') + _sysctl(system, 'vm.page_speculative_count')
    page_size = _sysctl(system, 'vm.pagesize')
    require(page_size in (4096, 16384), 'unsupported Darwin VM page size')
    budget = min(2 * 1024**3, pages * page_size // 4)
    require(budget >= 64 * MIB, 'insufficient current free memory for bounded SDK validation')
    return budget


def _task(proc, pid):
    value = _TaskInfo()
    size = proc.proc_pidinfo(pid, 4, 0, ctypes.byref(value), ctypes.sizeof(value))
    if size == 0 and ctypes.get_errno() in (errno.ESRCH, errno.ENOENT):
        return None
    require(size == ctypes.sizeof(value), 'cannot account for native compiler memory')
    return value


def _group_memory(proc, pid):
    members = (ctypes.c_int * 33)()
    ctypes.set_errno(0)
    count = proc.proc_listpgrppids(pid, members, ctypes.sizeof(members))
    require(0 <= count < len(members) and
            (count != 0 or ctypes.get_errno() in (0, errno.ESRCH, errno.ENOENT)),
            'native compiler process group unavailable or exceeds accounting ceiling')
    resident = 0
    for member in members[:count]:
        if member > 0:
            usage = _task(proc, member)
            if usage is not None:
                resident += usage.resident
    return resident


def _lower_limit(kind, ceiling):
    import resource

    soft, hard = resource.getrlimit(kind)
    finite = [ceiling] + [value for value in (soft, hard) if value != resource.RLIM_INFINITY]
    limit = min(finite)
    resource.setrlimit(kind, (limit, limit))


def _scratch_size(root):
    total = entries = 0
    for directory, folders, files in os.walk(root, followlinks=False):
        entries += len(folders) + len(files)
        require(entries <= 1024, 'native compiler scratch entry ceiling exceeded')
        for name in files:
            try:
                total += (Path(directory) / name).lstat().st_size
            except FileNotFoundError:
                # SDK tools may unlink a temporary file after walk enumerates it.
                continue
    require(total <= 128 * MIB, 'native compiler scratch byte ceiling exceeded')


def invoke(profile, toolname, args, output_limit=65536):
    """Run one pinned SDK tool without a shell, discovery, or GPU execution.

    Kernel CPU/data/file limits complement a 20ms aggregate process-group RSS
    watchdog (not a claim of instantaneous RSS isolation). Existing host limits
    are only lowered. A private cwd, HOME, module cache and TMPDIR are bounded.
    SIGTERM and default SIGINT unwind through owned process-group cleanup.
    The caller's SIGTERM handling resumes afterward; other termination signals
    have no cleanup guarantee here.
    """
    previous_mask = previous_handler = None
    handler_installed = terminated = cleaning = False

    def terminate(signum, frame):
        nonlocal terminated
        terminated = True
        if not cleaning:
            raise _SDKTerminated()

    try:
        _validate(profile)
        require(toolname in ('metal', 'metallib'), 'unknown official tool operation')
        require(type(args) in (list, tuple) and len(args) <= 128 and
                all(type(arg) is str and len(arg) <= 4096 and '\0' not in arg for arg in args) and
                sum(len(arg) for arg in args) <= 32768, 'invalid tool arguments')
        require(type(output_limit) is int and 1 <= output_limit <= MIB, 'invalid diagnostic byte ceiling')
        before, _ = _installation(profile)
        _host(profile)
        system, proc = _native()
        budget = _memory_budget(system)
        own_task = _task(proc, os.getpid())
        require(own_task is not None, 'cannot account for launcher address space')
        import resource

        # Darwin reserves very large shared-cache address ranges. Bound additional
        # address space, data allocations and observed RSS rather than pretending
        # an RLIMIT_AS below the already-mapped shared cache could work.
        address_limit = own_task.virtual + budget

        def limits():
            # Popen inherits the acquisition mask; do not leave SDK signals blocked.
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
            _lower_limit(resource.RLIMIT_AS, address_limit)
            _lower_limit(resource.RLIMIT_DATA, budget)
            _lower_limit(resource.RLIMIT_CPU, 30)
            _lower_limit(resource.RLIMIT_FSIZE, 64 * MIB)
            _lower_limit(resource.RLIMIT_CORE, 0)

        with tempfile.TemporaryDirectory(prefix='seen-metal-sdk-') as directory:
            env = {'LC_ALL': 'C', 'LANG': 'C', 'PATH': '/usr/bin:/bin',
                   'SDKROOT': profile['sdk']['path'], 'HOME': directory, 'TMPDIR': directory,
                   'CLANG_MODULE_CACHE_PATH': str(Path(directory) / 'modules')}
            started = time.monotonic()
            outputs = [bytearray(), bytearray()]
            child = None
            # Defer delivery until Popen has returned its owned child handle.
            previous_mask = signal.pthread_sigmask(
                signal.SIG_BLOCK, {signal.SIGTERM, signal.SIGINT})
            try:
                previous_handler = signal.getsignal(signal.SIGTERM)
                if previous_handler != signal.SIG_IGN:
                    signal.signal(signal.SIGTERM, terminate)
                    handler_installed = True
                child = subprocess.Popen([profile['tools'][toolname]['path'], *args],
                                         stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                         stderr=subprocess.PIPE, env=env, cwd=directory,
                                         start_new_session=True, preexec_fn=limits, close_fds=True)
                signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
                with selectors.DefaultSelector() as selector:
                    for index, stream in enumerate((child.stdout, child.stderr)):
                        os.set_blocking(stream.fileno(), False)
                        selector.register(stream, selectors.EVENT_READ, index)
                    while selector.get_map() or child.poll() is None:
                        require(time.monotonic() - started <= 45,
                                'official tool exceeded its wall-time deadline')
                        require(_group_memory(proc, child.pid) <= budget,
                                'official tool exceeded its current-memory-derived RSS budget')
                        _scratch_size(directory)
                        for key, _ in selector.select(0.02):
                            chunk = os.read(key.fd, min(8192, output_limit + 1))
                            if not chunk:
                                selector.unregister(key.fileobj)
                            else:
                                outputs[key.data].extend(chunk)
                                require(len(outputs[key.data]) <= output_limit,
                                        'official tool diagnostic output exceeds ceiling')
                    status = child.wait()
                    require(status >= 0, 'official tool terminated by a signal')
            finally:
                cleaning = True
                # Keep repeated termination from interrupting kill/reap or temp cleanup.
                signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGTERM, signal.SIGINT})
                if child is not None:
                    try:
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    finally:
                        child.wait()
                        child.stdout.close()
                        child.stderr.close()
                after, _ = _installation(profile)
                require(after == before, 'approved installation changed during invocation')
            return status, outputs[0].decode('utf-8', 'replace'), outputs[1].decode('utf-8', 'replace')
    except _SDKTerminated:
        # A returning caller handler still must not turn cancellation into success.
        raise ToolchainError('E_GPU_METAL_TOOLCHAIN: SDK invocation terminated') from None
    except ToolchainError:
        raise
    except (OSError, ValueError, TypeError, OverflowError, RuntimeError, ExpatError,
            subprocess.SubprocessError) as error:
        raise ToolchainError('E_GPU_METAL_TOOLCHAIN: SDK invocation failed: ' + str(error)) from error
    finally:
        try:
            if handler_installed:
                signal.signal(signal.SIGTERM, previous_handler)
            if terminated:
                os.kill(os.getpid(), signal.SIGTERM)
        finally:
            if previous_mask is not None:
                signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)


def verify_installation(profile):
    try:
        _validate(profile)
        identities, sdk = _installation(profile)
        _host(profile)
        status, output, diagnostic = invoke(profile, 'metal', ['--version'])
        lines = (output + '\n' + diagnostic).splitlines()
        lines = [line for line in lines if line.strip()]
        require(status == 0 and lines and lines[0] == profile['tools']['metal']['version_line'],
                'observed official metal version differs from approved version')
        after, _ = _installation(profile)
        require(after == identities, 'approved installation changed during version inspection')
        return {'schema': 'seen-metal-toolchain-evidence-v1', 'classification': 'compile-only',
                'platform': profile['platform'], 'targets': TARGETS, 'languages': LANGUAGES,
                'sdk': sdk, 'tools': {
                    'metal': {'path': profile['tools']['metal']['path'],
                              'sha256': profile['tools']['metal']['sha256'], 'version_line': lines[0],
                              'version_output': output, 'version_diagnostic': diagnostic},
                    'metallib': {'path': profile['tools']['metallib']['path'],
                                 'sha256': profile['tools']['metallib']['sha256']}},
                'gpu_execution_verified': False, 'hardware_verified': False}
    except ToolchainError:
        raise
    except (OSError, ValueError, TypeError, OverflowError, RuntimeError, ExpatError) as error:
        raise ToolchainError('E_GPU_METAL_TOOLCHAIN: installation verification failed: ' + str(error)) from error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--policy', type=Path, help='explicitly approved official SDK JSON policy')
    arguments = parser.parse_args()
    try:
        print(json.dumps(verify_installation(load_profile(arguments.policy)), sort_keys=True))
        return 0
    except ToolchainUnavailable as error:
        print(json.dumps({'schema': 'seen-metal-toolchain-evidence-v1', 'status': 'unsupported',
                          'official_validation': 'not-run', 'tool_execution': 'not-run',
                          'gpu_execution_verified': False, 'hardware_verified': False,
                          'reason': str(error)}, sort_keys=True))
        print(str(error), file=sys.stderr)
        return 126
    except ToolchainError as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
