#!/usr/bin/env python3
"""Build, authenticate and select the Linux x86-64 finite-address LLVM TSan SDK."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import signal
import shlex
import shutil
import subprocess
import sys
import tempfile

from finite_tsan_sdk_source import (ASSETS, PATCH_SHA256, ORIGINAL_SHA256, MODIFIED_SHA256,
                                    authenticate, digest, extract_sources, require)

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / 'scripts'
CACHE = ROOT / '.seen/toolchains/finite-tsan'
SCOPE = 'seen-finite-tsan-sdk'
RUNTIMES = ('libclang_rt.tsan-x86_64.a', 'libclang_rt.tsan-x86_64.a.syms',
            'libclang_rt.tsan_cxx-x86_64.a', 'libclang_rt.tsan_cxx-x86_64.a.syms')
HELPERS = ('finite_tsan_sdk.py', 'finite_tsan_sdk_source.py', 'finite_tsan_sdk.patch',
           'finite_tsan_sdk_control.c', 'install_finite_tsan_sdk.sh',
           'finite_tsan_sdk_exec.py')
CONFIG = [
    '-G', 'Ninja', '-DCMAKE_BUILD_TYPE=Release', '-DCMAKE_EXPORT_COMPILE_COMMANDS=ON',
    '-DCOMPILER_RT_DEFAULT_TARGET_ONLY=ON', '-DCOMPILER_RT_BUILD_BUILTINS=OFF',
    '-DCOMPILER_RT_BUILD_SANITIZERS=ON', '-DCOMPILER_RT_SANITIZERS_TO_BUILD=tsan',
    '-DCOMPILER_RT_BUILD_XRAY=OFF', '-DCOMPILER_RT_BUILD_LIBFUZZER=OFF',
    '-DCOMPILER_RT_BUILD_PROFILE=OFF', '-DCOMPILER_RT_BUILD_CTX_PROFILE=OFF',
    '-DCOMPILER_RT_BUILD_MEMPROF=OFF', '-DCOMPILER_RT_BUILD_ORC=OFF',
    '-DCOMPILER_RT_BUILD_GWP_ASAN=OFF', '-DCOMPILER_RT_INCLUDE_TESTS=OFF',
]


def canonical_json(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode()


def identity(value):
    return hashlib.sha256(canonical_json(value)).hexdigest()


def safe_path(path):
    path = Path(path)
    require(path.is_absolute() and str(path.resolve(strict=True)) == str(path),
            f'path is not absolute, canonical and non-symlink: {path}')
    require(not any(character in str(path) for character in '\n\r\x00'), 'unsafe path characters')
    return path


def inventory(root, follow_links=False):
    """Complete finite content inventory; published trees contain no symlinks."""
    root = safe_path(root)
    rows = {}
    total = 0
    directory_count = 0
    for directory, directories, filenames in os.walk(root, followlinks=follow_links):
        directory_count += 1
        require(directory_count <= 30000, 'SDK directory traversal exceeds ceiling')
        for name in directories:
            require(follow_links or not (Path(directory) / name).is_symlink(),
                    'resource directory is a symlink')
        for name in filenames:
            path = Path(directory) / name
            require(follow_links or not path.is_symlink(), 'resource file is a symlink')
            row = digest(path.resolve(strict=True) if follow_links else path)
            total += row['bytes']
            require(total <= 1024**3 and len(rows) < 30000, 'SDK tree exceeds bounded inventory')
            rows[str(path.relative_to(root))] = row
    require(bool(rows), 'empty SDK tree')
    return rows


def command(argv, timeout=30, **kwargs):
    if kwargs.pop('capture_output', False):
        kwargs.update(stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    with subprocess.Popen([str(arg) for arg in argv], start_new_session=True, **kwargs) as process:
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        finally:
            # Drain the owned command group, including configure/build children,
            # before scratch cleanup; never signal unrelated user processes.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
        result = subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)
        result.check_returncode()
        return result


def tool(path):
    actual = Path(path).resolve(strict=True)
    require(actual.is_file() and os.access(actual, os.X_OK), f'tool unavailable: {path}')
    return {'path': str(actual), **digest(actual)}


def inputs():
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64',
            'finite TSan SDK supports only Linux x86-64')
    clang = '/usr/bin/clang-20'
    version = command([clang, '--version'], capture_output=True, text=True).stdout
    require('clang version 20.1.2 ' in version, 'finite TSan SDK requires genuine LLVM 20.1.2')
    llvm_version = command(['/usr/bin/llvm-config-20', '--version'], capture_output=True, text=True).stdout.strip()
    require(llvm_version == '20.1.2', 'LLVM development package version differs')
    stock = Path(command([clang, '-print-resource-dir'], capture_output=True, text=True).stdout.strip()).resolve(strict=True)
    triple = command([clang, '-dumpmachine'], capture_output=True, text=True).stdout.strip()
    require(triple.startswith('x86_64-') and 'linux' in triple, 'compiler target differs')
    helpers = {name: digest(SCRIPTS / name, 1024**2) for name in HELPERS}
    require(helpers['finite_tsan_sdk.patch']['sha256'] == PATCH_SHA256, 'finite TSan patch identity differs')
    return {
        'schema': 1, 'platform': 'linux-x86_64', 'version': '20.1.2', 'triple': triple,
        'configuration': CONFIG, 'prefix_mapping': '/seen-finite-tsan', 'helpers': helpers,
        'tools': {name: tool(path) for name, path in {
            'clang': clang, 'clang++': '/usr/bin/clang++-20', 'llvm-config': '/usr/bin/llvm-config-20',
            'cmake': '/usr/bin/cmake', 'ninja': '/usr/bin/ninja', 'patch': '/usr/bin/patch',
        }.items()},
        'llvm_cmake': inventory(Path('/usr/lib/llvm-20/lib/cmake/llvm').resolve(strict=True), follow_links=True),
        'llvm_headers': inventory(Path('/usr/lib/llvm-20/include').resolve(strict=True), follow_links=True),
        'stock_resource': str(stock), 'stock_files': inventory(stock, follow_links=True),
        'sources': {name: {'bytes': size, 'sha256': sha} for name, (size, sha) in ASSETS.items()},
    }


def read_manifest(path, expected_sha256=None):
    safe_path(path)
    bounded = digest(path, 16 * 1024**2)
    require(bounded['bytes'] > 0, 'empty manifest')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        payload = stream.read(16 * 1024**2 + 1)
    require(len(payload) == bounded['bytes']
            and hashlib.sha256(payload).hexdigest() == bounded['sha256'],
            'SDK manifest changed while reading')
    require(expected_sha256 is None or bounded['sha256'] == expected_sha256,
            'SDK manifest bytes changed after wrapper binding')
    manifest = json.loads(payload)
    require(isinstance(manifest, dict) and set(manifest) == {
        'schema', 'identity', 'inputs', 'resource_files', 'source_files', 'authentication',
        'proof_files', 'controls', 'resource_directory'}, 'malformed finite TSan manifest')
    return manifest


def validate(manifest_path, compiler=None, expected=None, staging=False, source_auth=False,
             manifest_sha256=None):
    path = safe_path(manifest_path)
    manifest = read_manifest(path, expected_sha256=manifest_sha256)
    bound = inputs() if expected is None else expected
    key = identity(bound)
    require(manifest['schema'] == 1 and manifest['identity'] == key and manifest['inputs'] == bound,
            'SDK source/compiler/configuration identity differs')
    if not staging:
        require(path == CACHE / key / 'manifest.json', 'SDK manifest escaped identity-bound cache')
    require(manifest['resource_directory'] == 'resource', 'SDK resource directory differs')
    if compiler is not None:
        require(str(Path(compiler).resolve(strict=True)) == bound['tools']['clang']['path'],
                'selected SDK compiler identity differs')
    resource_dir = path.parent / 'resource'
    actual = inventory(resource_dir)
    require(actual == manifest['resource_files'], 'SDK resource bytes differ')
    replaced = {'lib/linux/' + name for name in RUNTIMES}
    require(set(actual) == set(bound['stock_files']) | replaced, 'SDK resource inventory differs')
    for name, row in bound['stock_files'].items():
        require(name in replaced or actual.get(name) == row, 'SDK changed a stock non-TSan resource')
    for name in RUNTIMES:
        require(actual['lib/linux/' + name]['bytes'] > 0, 'SDK runtime is empty')
        if name.endswith('.a'):
            with open(resource_dir / 'lib/linux' / name, 'rb') as archive:
                require(archive.read(8) == b'!<arch>\n', 'SDK runtime is not a real archive')
    require(inventory(path.parent / 'sources') == manifest['source_files'], 'SDK source inputs differ')
    for name, row in bound['sources'].items():
        require(manifest['source_files'].get(name) == row, 'SDK pinned source differs')
    require(inventory(path.parent / 'proof') == manifest['proof_files'], 'SDK control proof bytes differ')
    require(manifest['controls'] == {'clean': True, 'race': True, 'insufficient_limit': True},
            'SDK lacks real clean/race/limit control acceptance')
    if source_auth:
        require(authenticate(path.parent / 'sources') == manifest['authentication'],
                'SDK cached source authentication differs')
    return resource_dir


def compiler_limit():
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    cap = 2 * 1024**3
    if hard != resource.RLIM_INFINITY:
        cap = min(cap, hard)
    resource.setrlimit(resource.RLIMIT_AS, (cap, hard))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def build_environment():
    environment = dict(os.environ)
    for name in ('CFLAGS', 'CXXFLAGS', 'CPPFLAGS', 'LDFLAGS', 'CPATH', 'C_INCLUDE_PATH',
                 'CPLUS_INCLUDE_PATH', 'LIBRARY_PATH', 'LD_LIBRARY_PATH', 'LD_PRELOAD',
                 'CC', 'CXX', 'ASM', 'CCC_OVERRIDE_OPTIONS', 'CLANG_CONFIG_FILE_SYSTEM_DIR',
                 'CLANG_CONFIG_FILE_USER_DIR', 'CMAKE_PREFIX_PATH', 'CMAKE_TOOLCHAIN_FILE'):
        environment.pop(name, None)
    environment.update(SOURCE_DATE_EPOCH='1743619635', TZ='UTC', LC_ALL='C',
                       CMAKE_BUILD_PARALLEL_LEVEL='1', MAKEFLAGS='-j1')
    return environment


def kernel_snapshot():
    groups = [line[3:] for line in Path('/proc/self/cgroup').read_text().splitlines()
              if line.startswith('0::')]
    require(len(groups) == 1 and groups[0].startswith('/') and '..' not in Path(groups[0]).parts,
            'SDK unified aggregate scope is unavailable')
    group = Path('/sys/fs/cgroup' + groups[0])
    snapshot = {}
    for name in ('memory.max', 'memory.swap.max', 'pids.max', 'memory.events', 'pids.events'):
        with open(group / name, encoding='ascii') as stream:
            value = stream.read(65537)
        require(len(value) <= 65536, 'SDK kernel read-back exceeds ceiling')
        snapshot[name] = value.strip()
    require(snapshot['memory.max'].isdigit() and 0 < int(snapshot['memory.max']) <= 2 * 1024**3
            and snapshot['memory.swap.max'] == '0' and snapshot['pids.max'].isdigit()
            and 0 < int(snapshot['pids.max']) <= 24, 'SDK kernel aggregate controls differ')
    for name in ('memory.events', 'pids.events'):
        require(all(len(row.split()) == 2 and row.split()[1] == '0'
                    for row in snapshot[name].splitlines()), 'SDK kernel resource fault recorded')
    return snapshot


def controls(stage, bound, environment):
    work = Path(tempfile.mkdtemp(prefix='finite-tsan-control-', dir=os.environ['SEEN_ARTIFACT_ROOT']))
    proof = stage / 'proof'
    proof.mkdir()
    try:
        argv = ['/usr/bin/clang-20', '--no-default-config', '-std=c11', '-D_GNU_SOURCE', '-O1', '-g',
                '-march=x86-64', '-fno-omit-frame-pointer', '-fsanitize=thread', '-resource-dir',
                stage / 'resource', '-fPIE', '-pie', '-pthread', SCRIPTS / 'finite_tsan_sdk_control.c',
                '-o', work / 'control']
        with open(work / 'compile.log', 'w') as log:
            log.write(shlex.join([str(arg) for arg in argv]) + '\n')
            log.write('  Sanitizer: thread\n')
            log.flush()
            command(argv, timeout=60, env=environment, preexec_fn=compiler_limit, stdout=log, stderr=log)
        shutil.copyfile(work / 'compile.log', proof / 'control-compile.log')
        results = {}
        for mode in ('clean', 'race'):
            with open(proof / (mode + '.out'), 'wb') as out, open(proof / (mode + '.err'), 'wb') as err:
                result = subprocess.run(['timeout', '--foreground', '--kill-after=5s', '30s',
                    'bash', str(SCRIPTS / 'run_asan_in_hard_memory_scope.sh'), '--sanitizer', 'thread',
                    '--target-root', str(work), '--compile-log', str(work / 'compile.log'),
                    '--', str(work / 'control'), mode], env=environment, stdout=out, stderr=err, timeout=40)
            results[mode] = result.returncode
        clean = (proof / 'clean.out').read_text()
        race = (proof / 'race.err').read_text()
        require(results['clean'] == 0 and
                'TSAN_CONTROL value=2 soft=140737488355328 hard=140737488355328 aslr_disabled=0' in clean,
                'finite TSan clean control failed; see ' + str(proof))
        require(results['race'] not in (0, 124, 137) and 'ThreadSanitizer: data race' in race,
                'finite TSan real two-writer race control failed')

        def insufficient():
            resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
            resource.setrlimit(resource.RLIMIT_AS, (64 * 1024**4, 64 * 1024**4))
            if resource.getrlimit(resource.RLIMIT_AS) != (64 * 1024**4, 64 * 1024**4):
                os._exit(125)

        negative = subprocess.run([str(work / 'control'), 'clean'], env=dict(environment,
            TSAN_OPTIONS='halt_on_error=1:exitcode=66:report_signal_unsafe=1'),
            preexec_fn=insufficient, capture_output=True, timeout=30)
        (proof / 'insufficient.out').write_bytes(negative.stdout)
        (proof / 'insufficient.err').write_bytes(negative.stderr)
        require(negative.returncode != 0 and b'finite address-space limit must cover' in negative.stderr
                and b'TSAN_CONTROL' not in negative.stdout, '64 TiB admission control failed')
        results['insufficient_limit'] = negative.returncode
        kernel = kernel_snapshot()
        (proof / 'observations.json').write_bytes(canonical_json({
            'statuses': results, 'native': digest(work / 'control'), 'kernel': kernel,
            'seen_consumer_verified': False}) + b'\n')
        return {'clean': True, 'race': True, 'insufficient_limit': True}
    finally:
        shutil.rmtree(work)


def build():
    command(['bash', SCRIPTS / 'run_capped_regression.sh', '--verify-platform-active', SCOPE])
    require(os.environ.get('SEEN_OPT_JOBS') == '1' and os.environ.get('SEEN_JOBS') == '1',
            'SDK build requires serial scope')
    kernel_snapshot()
    for directory in (ROOT / '.seen', CACHE.parent, CACHE):
        safe_path(directory.parent)
        if not directory.exists():
            directory.mkdir(mode=0o700)
        safe_path(directory)
    lock_fd = os.open(CACHE / '.lock', os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(lock_fd, 'w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        bound = inputs()
        key = identity(bound)
        published = CACHE / key
        # Cached payloads cannot authorize their own rebuilt runtime hashes.
        # Always produce fresh controls and a publication digest for this job.
        stage = Path(tempfile.mkdtemp(prefix='.building-', dir=CACHE))
        try:
            sources = stage / 'sources'
            sources.mkdir()
            authentication = authenticate(sources, download=True)
            source_files = inventory(sources)
            work = stage / 'work'
            work.mkdir()
            extract_sources(sources, work)
            source = work / 'compiler-rt-20.1.2.src'
            patched = source / 'lib/tsan/rtl/tsan_platform_linux.cpp'
            require(digest(patched)['sha256'] == ORIGINAL_SHA256, 'upstream TSan source differs')
            with open(SCRIPTS / 'finite_tsan_sdk.patch', 'rb') as patch:
                command(['/usr/bin/patch', '--batch', '--forward', '--fuzz=0', '-p1', '-d', source],
                        stdin=patch, stdout=sys.stderr, stderr=sys.stderr)
            require(digest(patched)['sha256'] == MODIFIED_SHA256, 'patched TSan source differs')
            environment = build_environment()
            mapping = shlex.join(f'-f{kind}-prefix-map={stage}=/seen-finite-tsan'
                                 for kind in ('file', 'debug', 'macro'))
            configure = ['/usr/bin/cmake', '-S', source, '-B', work / 'build', *CONFIG,
                '-DCMAKE_MAKE_PROGRAM=/usr/bin/ninja', '-DCMAKE_C_COMPILER=/usr/bin/clang-20',
                '-DCMAKE_CXX_COMPILER=/usr/bin/clang++-20', '-DCMAKE_ASM_COMPILER=/usr/bin/clang-20',
                '-DCMAKE_C_COMPILER_TARGET=' + bound['triple'], '-DCMAKE_CXX_COMPILER_TARGET=' + bound['triple'],
                '-DLLVM_COMMON_CMAKE_UTILS=' + str(work / 'cmake-20.1.2.src'),
                '-DLLVM_CMAKE_DIR=/usr/lib/llvm-20/lib/cmake/llvm',
                '-DLLVM_MAIN_SRC_DIR=' + str(work / 'llvm-20.1.2.src'),
                '-DCMAKE_C_FLAGS=' + mapping + ' --no-default-config',
                '-DCMAKE_CXX_FLAGS=' + mapping + ' --no-default-config',
                '-DCMAKE_ASM_FLAGS=' + mapping + ' --no-default-config']
            logs = stage / 'logs'
            logs.mkdir()
            with open(logs / 'configure.log', 'wb') as log:
                command(configure, timeout=120, env=environment, preexec_fn=compiler_limit, stdout=log, stderr=log)
            with open(logs / 'build.log', 'wb') as log:
                command(['timeout', '--foreground', '--kill-after=5s', '540s', '/usr/bin/cmake',
                         '--build', work / 'build', '--parallel', '1', '--target', 'tsan'],
                        timeout=550, env=environment, preexec_fn=compiler_limit, stdout=log, stderr=log)
            shutil.copytree(bound['stock_resource'], stage / 'resource', symlinks=False)
            for name in RUNTIMES:
                built = work / 'build/lib/linux' / name
                digest(built, 128 * 1024**2)
                shutil.copyfile(built, stage / 'resource/lib/linux' / name)
            accepted = controls(stage, bound, environment)
            # Detect compiler/stock/header/source changes during the actual build.
            require(inputs() == bound, 'SDK build inputs changed during build')
            require(inventory(sources) == source_files, 'SDK sources changed during build')
            manifest = {'schema': 1, 'identity': key, 'inputs': bound,
                'resource_directory': 'resource', 'resource_files': inventory(stage / 'resource'),
                'source_files': source_files, 'authentication': authentication,
                'proof_files': inventory(stage / 'proof'), 'controls': accepted}
            payload = canonical_json(manifest) + b'\n'
            published_sha256 = hashlib.sha256(payload).hexdigest()
            (stage / 'manifest.json').write_bytes(payload)
            validate(stage / 'manifest.json', expected=bound, staging=True, source_auth=True,
                     manifest_sha256=published_sha256)
            kernel_snapshot()
            shutil.rmtree(work)
            if published.exists() or published.is_symlink():
                if published.is_symlink():
                    published.unlink()
                else:
                    shutil.rmtree(published)
            # Atomic visibility, not a power-loss durability claim.
            os.rename(stage, published)
            return published / 'manifest.json', published_sha256
        except BaseException:
            diagnostics = Path(os.environ['SEEN_ARTIFACT_ROOT']) / 'finite-tsan-failure'
            diagnostics.mkdir(exist_ok=True)
            remaining = 1024**2
            for folder in ('logs', 'proof'):
                for name in ('configure.log', 'build.log', 'control-compile.log',
                             'clean.out', 'clean.err', 'race.out', 'race.err',
                             'insufficient.out', 'insufficient.err'):
                    path = stage / folder / name
                    if remaining <= 0 or not path.is_file() or path.is_symlink():
                        continue
                    with path.open('rb') as source:
                        count = min(remaining, 128 * 1024)
                        source.seek(max(0, path.stat().st_size - count))
                        content = source.read(count)
                    (diagnostics / (folder + '-' + name)).write_bytes(content)
                    remaining -= len(content)
            shutil.rmtree(stage)
            print('finite TSan SDK bounded diagnostics: ' + str(diagnostics), file=sys.stderr)
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='operation', required=True)
    install = sub.add_parser('build')
    install.add_argument('--output', required=True, type=Path)
    verify = sub.add_parser('validate')
    verify.add_argument('manifest', type=Path)
    verify.add_argument('--compiler')
    verify.add_argument('--source-auth', action='store_true')
    verify.add_argument('--manifest-sha256', required=True)
    args = parser.parse_args()
    if args.operation == 'build':
        manifest, published_sha256 = build()
        validate(manifest, source_auth=True, manifest_sha256=published_sha256)
        # Provisioning receives only this validated path; never source shell data.
        require(args.output.is_absolute() and not args.output.is_symlink(), 'unsafe output path')
        with open(args.output, 'x', encoding='utf-8') as out:
            json.dump({'manifest': str(manifest), 'sha256': published_sha256}, out)
            out.write('\n')
        print(manifest)
    else:
        print(validate(args.manifest, compiler=args.compiler, source_auth=args.source_auth,
                       manifest_sha256=args.manifest_sha256))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, IndexError, StopIteration,
            subprocess.SubprocessError) as error:
        raise SystemExit('RESOURCE STOP: finite TSan SDK: ' + str(error))
