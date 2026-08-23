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

sudo -n apt-get update
sudo -n apt-get install --yes --no-install-recommends \
    bubblewrap ripgrep libvulkan-dev llvm-20 clang-20 lld-20 \
    libclang-rt-20-dev
[ -x /usr/bin/bwrap ] || fail "Bubblewrap was not installed"
[ "$(command -v rg || true)" = "/usr/bin/rg" ] || fail "ripgrep was not installed"
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
if ! printf '%s\n' \
    'define ptr @seen_ci_gep_nuw(ptr %base, i64 %index) {' \
    'entry:' \
    '  %result = getelementptr inbounds nuw i64, ptr %base, i64 %index' \
    '  ret ptr %result' \
    '}' | "$llvm_root/llvm-as" -o /dev/null -; then

    fail "pinned LLVM does not accept Seen's required GEP no-wrap IR dialect"
fi
if ! printf '%s\n' 'int main(void) { return 0; }' | "$llvm_root/clang" \
    -x c - -o /dev/null -fuse-ld=lld -fprofile-instr-generate \
    -fcoverage-mapping -fsanitize=undefined -fno-omit-frame-pointer -g; then

    fail "pinned LLVM instrumentation runtimes cannot compile and link"
fi
printf '%s\n' "$llvm_root" >> "$GITHUB_PATH"
PATH="$llvm_root:$PATH"
export PATH

work_root=$(mktemp -d "$RUNNER_TEMP/seen-ci-host.XXXXXX") ||
    fail "could not create runner-local provisioning root"
cleanup() {
    local status=$?
    case "$work_root" in
        "$RUNNER_TEMP"/seen-ci-host.*)
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

# The committed 0.10.1 bootstrap seeds predate the Linux --as-needed link
# policy and carry an unused SDL3 DT_NEEDED entry. Noble has no SDL3 package.
# Admit a loader-only bridge solely after proving that neither seed imports an
# SDL symbol. The rebuilt compiler drops this dependency at its source linker.
needs_sdl3_bridge=0
for frozen in bootstrap/stage1_frozen bootstrap/stage1_frozen_v3; do
    [ -f "$frozen" ] && [ ! -L "$frozen" ] ||
        fail "frozen bootstrap input is missing or unsafe: $frozen"
    if /usr/bin/readelf -d "$frozen" | /usr/bin/grep -Fq 'libSDL3.so.0'; then
        needs_sdl3_bridge=1
        if /usr/bin/nm -D --undefined-only "$frozen" |
            /usr/bin/grep -Eq '(^|[[:space:]])SDL_[[:alnum:]_]+'; then

            fail "frozen bootstrap imports SDL3 symbols; loader bridge refused"
        fi
    fi
done

if [ "$needs_sdl3_bridge" -eq 1 ]; then
    compat_root=$(mktemp -d "$RUNNER_TEMP/seen-bootstrap-compat.XXXXXX") ||
        fail "could not create bootstrap compatibility root"
    printf '%s\n' 'void seen_ci_sdl3_loader_bridge(void) {}' |
        /usr/bin/cc -x c -shared -Wl,-soname,libSDL3.so.0 - \
            -o "$compat_root/libSDL3.so.0" ||
        fail "could not build the frozen-bootstrap SDL3 loader bridge"
    ln -s libSDL3.so.0 "$compat_root/libSDL3.so" ||
        fail "could not expose the frozen-bootstrap SDL3 linker name"
    [ -f "$compat_root/libSDL3.so.0" ] &&
        [ -L "$compat_root/libSDL3.so" ] ||
        fail "SDL3 loader bridge failed read-back"
    printf 'LD_LIBRARY_PATH=%s\n' "$compat_root" >> "$GITHUB_ENV"
    printf 'LIBRARY_PATH=%s\n' "$compat_root" >> "$GITHUB_ENV"
fi

echo "PASS: Ubuntu CI host tools, scoped Bubblewrap namespace, and bootstrap compatibility"
