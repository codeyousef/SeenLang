#!/usr/bin/env bash
# Native CPU-only Ubuntu ARM64 qualification tools; never executes x64 inputs.
set -euo pipefail
ROOT_DIR=$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)
cd -- "$ROOT_DIR"
fail() {
    echo "ci-arm64-host-provision: core.001a.invalid: $*" >&2
    exit 1
}
[ "$#" = 0 ] || fail "no arguments are accepted"
[ "${GITHUB_ACTIONS:-}" = true ] || fail "provisioning is restricted to GitHub Actions"
[ "${RUNNER_OS:-}" = Linux ] && [ "${RUNNER_ARCH:-}" = ARM64 ] ||
    fail "a native Linux ARM64 Actions runner is required"
[ "$(uname -s)" = Linux ] || fail "the real host must be Linux"
case "$(uname -m)" in
    aarch64|arm64) ;;
    *) fail "the real host must be ARM64; emulation/x64 inputs are not supported" ;;
esac
[ "$(dpkg --print-architecture)" = arm64 ] || fail "native dpkg architecture must be arm64"
for name in RUNNER_TEMP GITHUB_ENV GITHUB_PATH; do
    value=${!name:-}
    case "$value" in
        /*) ;;
        *) fail "$name must be absolute" ;;
    esac
    case "$value" in
        *:*|*$'\n'*|*$'\r'*) fail "$name contains an unsafe character" ;;
    esac
done
[ -d "$RUNNER_TEMP" ] && [ ! -L "$RUNNER_TEMP" ] || fail "RUNNER_TEMP must be a real directory"
[ -f /etc/os-release ] || fail "runner OS identity is unavailable"
# shellcheck source=/etc/os-release
source /etc/os-release
[ "${ID:-}" = ubuntu ] && [ "${VERSION_ID:-}" = 24.04 ] ||
    fail "Ubuntu 24.04 runner is required"

# This profile owns the package version only. Its x64 tool hashes/paths are NOT
# ARM identities and must never be applied to these native Ubuntu packages.
llvm_package_version=$(python3 - <<'PY'
import json
import re
from pathlib import Path
version = json.loads(Path('docs/architecture/amdgpu-toolchain.json').read_text())['profiles']['linux-x86_64']['package_version']
if not isinstance(version, str) or not re.fullmatch(r'1:20\.[0-9]+\.[0-9]+-[0-9A-Za-z.+~]+', version):
    raise SystemExit('invalid pinned LLVM 20 package version')
print(version)
PY
) || fail "pinned LLVM package version is unavailable"
llvm_packages=(llvm-20 clang-20 lld-20 libllvm20 libclang-rt-20-dev)
pinned_packages=()
for package in "${llvm_packages[@]}"; do
    pinned_packages+=("$package:arm64=$llvm_package_version")
done
# Use signed Ubuntu repository metadata and authenticated package downloads.
# Missing noble-updates/universe ARM builds fail; no third-party installer,
# unauthenticated package, source build, newer-version fallback or GPU SDK.
apt_options=(-o APT::Get::AllowUnauthenticated=false
    -o Acquire::AllowInsecureRepositories=false
    -o Acquire::AllowDowngradeToInsecureRepositories=false)
sudo -n apt-get "${apt_options[@]}" update
sudo -n apt-get "${apt_options[@]}" install --yes --no-install-recommends \
    bubblewrap ripgrep strace apparmor python3 build-essential binutils \
    libc6-dev pkg-config procps util-linux coreutils git
# Reinstall even pre-existing pinned packages so this invocation authenticates
# their downloads instead of trusting whatever a runner image left installed.
sudo -n apt-get "${apt_options[@]}" install --yes --no-install-recommends --reinstall --allow-downgrades \
    "${pinned_packages[@]}"
for package in "${llvm_packages[@]}"; do
    identity=$(dpkg-query -W -f='${Status}\t${Architecture}\t${Version}' "$package:arm64") ||
        fail "installed package identity is unavailable: $package"
    [ "$identity" = "install ok installed"$'\t'"arm64"$'\t'"$llvm_package_version" ] ||
        fail "installed native pinned package differs: $package"
done
for tool in bwrap strace nm readelf cc make python3; do
    [ -x "/usr/bin/$tool" ] || fail "native prerequisite is unavailable: $tool"
done
[ "$(command -v rg || true)" = /usr/bin/rg ] || fail "ripgrep was not installed"
[ -x /usr/sbin/apparmor_parser ] || fail "AppArmor parser is unavailable"

llvm_root="$RUNNER_TEMP/seen-llvm-20/bin"
mkdir -p -- "$llvm_root"
for llvm_tool in clang clang++ opt llvm-as llvm-link llc llvm-profdata \
    llvm-cov llvm-nm llvm-ar llvm-dis llvm-objdump ld.lld lld wasm-ld; do
    versioned_tool="/usr/bin/${llvm_tool}-20"
    [ -x "$versioned_tool" ] || fail "pinned LLVM 20 tool is unavailable: $versioned_tool"
    ln -s "$versioned_tool" "$llvm_root/$llvm_tool" ||
        fail "could not expose pinned LLVM 20 tool: $llvm_tool"
done
# Inspect real ELF headers BEFORE executing any selected LLVM tool. A foreign
# binary executable through binfmt/QEMU must not pass native host admission.
python3 - "$llvm_root" <<'PY'
import struct
import sys
from pathlib import Path

def native_elf(path):
    with path.open('rb') as stream:
        header = stream.read(20)
    if (len(header) != 20 or header[:6] != b'\x7fELF\x02\x01' or
            struct.unpack_from('<H', header, 18)[0] != 183):
        raise SystemExit(f'not a native ELF64 little-endian AArch64 tool: {path}')

for path in sorted(Path(sys.argv[1]).iterdir()):
    native_elf(path.resolve(strict=True))
native_elf(Path('/proc/self/exe').resolve(strict=True))
PY
# The pinned package owns this resource path. Actual Clang version/default-target
# and resource queries run later inside capped native qualification, never as
# uncontained provisioning probes.
resource_root=/usr/lib/llvm-20/lib/clang/20
python3 - "$resource_root" <<'PY'
import os
import re
import struct
import subprocess
import sys
from pathlib import Path
root = Path(sys.argv[1]) / 'lib/linux'
for name in ('asan', 'ubsan_standalone'):
    library = root / f'libclang_rt.{name}-aarch64.so'
    with library.open('rb') as stream:
        header = stream.read(20)
    if (len(header) != 20 or header[:6] != b'\x7fELF\x02\x01' or
            struct.unpack_from('<H', header, 18)[0] != 183):
        raise SystemExit(f'compiler-rt shared runtime is not native AArch64: {library}')
for name in ('asan', 'asan_cxx', 'asan-preinit', 'ubsan_standalone', 'builtins'):
    archive = root / f'libclang_rt.{name}-aarch64.a'
    result = subprocess.run(['/usr/bin/readelf', '-h', str(archive)],
                            check=True, capture_output=True, text=True,
                            env={**os.environ, 'LC_ALL': 'C'})
    machines = re.findall(r'^\s*Machine:\s*(.+)$', result.stdout, re.MULTILINE)
    if not machines or any(machine.strip() != 'AArch64' for machine in machines):
        raise SystemExit(f'compiler-rt archive is not wholly native AArch64: {archive}')
PY

mkdir -p -- "$ROOT_DIR/.seen/toolchains"
work_root=$(mktemp -d "$ROOT_DIR/.seen/toolchains/ci-arm64-host.XXXXXX") ||
    fail "could not create project-local provisioning root"
cleanup() {
    local status=$?
    case "$work_root" in
        "$ROOT_DIR"/.seen/toolchains/ci-arm64-host.*)
            [ -d "$work_root" ] && [ ! -L "$work_root" ] && rm -rf -- "$work_root"
            ;;
        *) return 1 ;;
    esac
    return "$status"
}
trap cleanup EXIT
bash "$ROOT_DIR/scripts/ci_linux_namespace_preflight.sh" "$work_root"
# Beyond the shared mount/read-back policy, prove that this native process can
# enter new user, PID, mount, IPC and UTS namespaces with a real /proc mapping.
python3 - "$work_root/probe" <<'PY'
import os
import subprocess
import sys
names = ('user', 'pid', 'mnt', 'ipc', 'uts')
host = [os.readlink(f'/proc/self/ns/{name}') for name in names]
subprocess.run([
    '/usr/bin/bwrap', '--die-with-parent', '--unshare-user', '--unshare-pid',
    '--unshare-ipc', '--unshare-uts', '--bind', '/', '/', '--dev-bind', '/dev', '/dev',
    '--proc', '/proc', '--ro-bind', '/sys', '/sys', '--bind', sys.argv[1], '/tmp',
    '--', '/usr/bin/python3', '-c', '''
import os, sys
names = ('user', 'pid', 'mnt', 'ipc', 'uts')
for name, previous in zip(names, sys.argv[1:]):
    if os.readlink(f'/proc/self/ns/{name}') == previous:
        raise SystemExit(f'native namespace did not change: {name}')
if os.readlink('/proc/self/exe') != os.readlink(f'/proc/{os.getpid()}/exe'):
    raise SystemExit('native process /proc mapping differs')
''', *host], check=True)
PY
printf '%s\n' "$llvm_root" >> "$GITHUB_PATH"
export PATH="$llvm_root:$PATH"
echo "PASS: authenticated pinned native Ubuntu ARM64 CPU tools and live process namespaces (not milestone certification)"
