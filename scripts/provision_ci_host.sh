#!/usr/bin/env bash
# Provision the exact Ubuntu-hosted tools required by the contained CI gate.

set -euo pipefail

fail() {
    echo "ci-host-provision: core.001a.invalid: $*" >&2
    exit 1
}

[ "${GITHUB_ACTIONS:-}" = "true" ] ||
    fail "provisioning is restricted to GitHub Actions"
[ "${RUNNER_OS:-}" = "Linux" ] || fail "Linux runner is required"
case "${RUNNER_TEMP:-}" in
    /*) ;;
    *) fail "RUNNER_TEMP must be absolute" ;;
esac
case "$RUNNER_TEMP" in
    *:*|*$'\n'*|*$'\r'*) fail "RUNNER_TEMP contains an unsafe character" ;;
esac
[ -n "${GITHUB_ENV:-}" ] && [ "${GITHUB_ENV#/}" != "$GITHUB_ENV" ] ||
    fail "GITHUB_ENV must be absolute"
[ -n "${GITHUB_PATH:-}" ] && [ "${GITHUB_PATH#/}" != "$GITHUB_PATH" ] ||
    fail "GITHUB_PATH must be absolute"
[ -f /etc/os-release ] || fail "runner OS identity is unavailable"
# shellcheck source=/etc/os-release
source /etc/os-release
[ "${ID:-}" = "ubuntu" ] && [ "${VERSION_ID:-}" = "24.04" ] ||
    fail "Ubuntu 24.04 runner is required"

spirv_package_version=$(python3 -c 'import json; from pathlib import Path; print(json.loads(Path("docs/architecture/spirv-toolchain.json").read_text())["profiles"]["linux-x86_64"]["package_version"])')
llvm_package_version=$(python3 -c 'import json; from pathlib import Path; print(json.loads(Path("docs/architecture/amdgpu-toolchain.json").read_text())["profiles"]["linux-x86_64"]["package_version"])')

sudo -n apt-get update
sudo -n apt-get install --yes --no-install-recommends \
    bubblewrap ripgrep strace libvulkan-dev mesa-vulkan-drivers libc6-dev-arm64-cross \
    "llvm-20=$llvm_package_version" "clang-20=$llvm_package_version" "lld-20=$llvm_package_version" \
    "libllvm20=$llvm_package_version" "libclang-rt-20-dev=$llvm_package_version" "spirv-tools=$spirv_package_version"
python3 scripts/check_gpu_amdgpu_toolchain.py >/dev/null || fail "pinned LLVM AMDGPU toolchain identity differs"
[ -x /usr/bin/spirv-val ] || fail "SPIR-V validator was not installed"
python3 scripts/check_gpu_spirv_toolchain.py >/dev/null || fail "pinned SPIR-V toolchain identity differs"
ptx_root="$RUNNER_TEMP/seen-ptx-toolchain"
mkdir -p -- "$ptx_root"
python3 scripts/install_gpu_ptx_toolchain.py "$ptx_root/ptxas" || fail "pinned PTX assembler installation failed"
printf 'SEEN_PTXAS=%s\n' "$ptx_root/ptxas" >> "$GITHUB_ENV"

[ -x /usr/bin/bwrap ] || fail "Bubblewrap was not installed"
[ "$(command -v rg || true)" = "/usr/bin/rg" ] || fail "ripgrep was not installed"
[ -x /usr/bin/strace ] || fail "strace SDK-discovery observer was not installed"
[ -e /usr/lib/x86_64-linux-gnu/libvulkan.so ] ||
    fail "Vulkan development linker input was not installed"
[ -x /usr/sbin/apparmor_parser ] || fail "AppArmor parser is unavailable"
[ -x /usr/bin/nm ] || fail "nm is unavailable"
[ -x /usr/bin/readelf ] || fail "readelf is unavailable"
[ -x /usr/bin/cc ] || fail "C compiler is unavailable"

llvm_root="$RUNNER_TEMP/seen-llvm-20/bin"
mkdir -p -- "$llvm_root"
for llvm_tool in clang clang++ opt llvm-as llvm-link llc llvm-profdata \
    llvm-cov llvm-nm llvm-ar llvm-dis llvm-objdump ld.lld lld wasm-ld; do

    versioned_tool="/usr/bin/${llvm_tool}-20"
    [ -x "$versioned_tool" ] ||
        fail "pinned LLVM 20 tool is unavailable: $versioned_tool"
    ln -s "$versioned_tool" "$llvm_root/$llvm_tool" ||
        fail "could not expose pinned LLVM 20 tool: $llvm_tool"
done
case "$("$llvm_root/llvm-as" --version | sed -n '1,2p')" in
    *'LLVM version 20.'*) ;;
    *) fail "LLVM assembler is not the pinned major version 20" ;;
esac
printf '%s\n' "$llvm_root" >> "$GITHUB_PATH"
PATH="$llvm_root:$PATH"
export PATH

mkdir -p -- .seen/toolchains
work_root=$(mktemp -d "$PWD/.seen/toolchains/ci-host.XXXXXX") ||
    fail "could not create runner-local provisioning root"
cleanup() {
    local status=$?
    case "$work_root" in
        "$PWD"/.seen/toolchains/ci-host.*)
            [ -d "$work_root" ] && [ ! -L "$work_root" ] &&
                rm -rf -- "$work_root"
            ;;
        *) return 1 ;;
    esac
    return "$status"
}
trap cleanup EXIT


profile="$work_root/usr.bin.bwrap"
printf '%s\n' \
    'abi <abi/4.0>,' \
    '' \
    'include <tunables/global>' \
    '' \
    '/usr/bin/bwrap flags=(default_allow) {' \
    '  userns,' \
    '}' > "$profile"
sudo -n /usr/sbin/apparmor_parser --replace "$profile"

probe_root="$work_root/probe"
mkdir -p -- "$probe_root"
host_identity=$(stat -c '%d:%i' "$probe_root")
mapped_identity=$(/usr/bin/bwrap --die-with-parent --bind / / \
    --dev-bind /dev /dev --proc /proc --ro-bind /sys /sys \
    --bind "$probe_root" /tmp -- /usr/bin/stat -c '%d:%i' /tmp) ||
    fail "Bubblewrap namespace probe failed"
[ "$mapped_identity" = "$host_identity" ] ||
    fail "Bubblewrap bind mapping failed read-back"

# SDK compilation and the conditional loader-only bridge must not precede
# aggregate containment. The platform runner freshly admits this 2 GiB ceiling,
# verifies swap/task controls, and maps all compiler scratch into the project.
compat_root=$(mktemp -d "$PWD/.seen/toolchains/ci-bootstrap-compat.XXXXXX") ||
    fail "could not create bootstrap compatibility root"
if ! SEEN_MEMORY_GUARD_RSS_KB=2097152 SEEN_MEMORY_GUARD_CGROUP_STOP_KB=2097152 \
    SEEN_MAIN_VMEM_KB=2097152 SEEN_OPT_VMEM_KB=1048576 SEEN_MEMORY_LIMIT_BYTES=2147483648 \
    bash scripts/verify_ci_native_host_inputs.sh "$compat_root"; then
    rm -rf -- "$compat_root"
    fail "scoped native host input verification failed"
fi
if [ -f "$compat_root/libSDL3.so.0" ]; then
    printf 'LD_LIBRARY_PATH=%s\n' "$compat_root" >> "$GITHUB_ENV"
    printf 'LIBRARY_PATH=%s\n' "$compat_root" >> "$GITHUB_ENV"
else
    rmdir -- "$compat_root"
fi

# Noble ships the official validator as a static archive, not the shared C ABI
# required by opt-in runtime artifact validation. Relink those exact pinned
# vendor bytes; do not replace validation with a CLI success or a local shim.
validator_root=$(mktemp -d "$PWD/.seen/toolchains/ci-spirv-validator.XXXXXX") ||
    fail "could not create the runtime validator installation root"
timeout --kill-after=5s 30s python3 tests/gpu_artifacts/spirv_validator_install_contract.py "$validator_root"
if ! SEEN_MEMORY_GUARD_RSS_KB=2097152 \
    bash scripts/run_capped_regression.sh --platform seen-spirv-validator-sdk -- \
        timeout --kill-after=5s 360s python3 scripts/install_gpu_spirv_validator.py \
        "$validator_root/libSPIRV-Tools-shared.so" > "$validator_root/install.log"; then
    cat -- "$validator_root/install.log" >&2
    fail "pinned official runtime validator installation failed"
fi
# Install through the normal loader cache, not a global SDK LD_LIBRARY_PATH:
# CPU-only compiler processes must not probe a GPU SDK directory on startup.
sudo -n install -m 0755 -- "$validator_root/libSPIRV-Tools-shared.so" \
    /usr/local/lib/libSPIRV-Tools-shared.so
sudo -n ldconfig


echo "PASS: Ubuntu CI host tools, scoped Bubblewrap namespace, and bootstrap compatibility"
