#!/usr/bin/env bash
# Build a macOS arm64 release archive on Linux with a user-supplied Apple SDK.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
root_dir="$(cd "$script_dir/.." && pwd -P)"
version="${1:-}"
sdk="${SEEN_MACOS_SDKROOT:-}"
builder="${SEEN_MACOS_BUILDER:-$root_dir/compiler_seen/target/seen}"
go_bin="${SEEN_GO:-}"
output_dir="${SEEN_MACOS_OUTPUT_DIR:-$root_dir/dist/platform-input}"

die() { printf 'macOS release build: %s\n' "$*" >&2; exit 1; }
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'expected release version'
[[ "$sdk" == /* && -d "$sdk/usr/include" && -f "$sdk/SDKSettings.json" ]] ||
    die 'SEEN_MACOS_SDKROOT must be a complete absolute SDK path'
[[ -x "$builder" && ! -L "$builder" ]] || die 'validated builder is missing'
[[ -x "$go_bin" ]] || die 'SEEN_GO must name the pinned Go executable'
[[ "$(uname -s)" == Linux ]] || die 'a Linux hard scope is required'

if [[ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ]]; then
    exec "$script_dir/run_with_project_artifacts.sh" macos-arm64-release --keep-on-failure -- \
        "$script_dir/run_in_hard_memory_scope.sh" \
        --label 'macOS arm64 release cross-build' --timeout-secs 2400 -- \
        "$0" "$@"
fi
"$script_dir/run_in_hard_memory_scope.sh" --verify-only >/dev/null ||
    die 'hard scope read-back failed'
[[ "${SEEN_JOBS:-1}" == 1 && "${SEEN_OPT_JOBS:-1}" == 1 ]] ||
    die 'compiler and optimizer workers must be serial'

tools_dir="$root_dir/.seen/agent-tools/macos-cross-tools"
mkdir -p "$tools_dir"
if [[ ! -e "$tools_dir/clang" ]]; then
    ln -s "$script_dir/macos_cross_clang.sh" "$tools_dir/clang"
fi
[[ "$(readlink -f "$tools_dir/clang")" == "$script_dir/macos_cross_clang.sh" ]] ||
    die 'cross-clang tool entry has changed'

mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd -P)"
compiler_out="$output_dir/seen-macos-arm64"
package_out="$output_dir/seen-pkg-macos-arm64"
export SEEN_COMPILER_SOURCE_ROOT="$root_dir"
export PATH="$tools_dir:$PATH"
export SEEN_MACOS_SDKROOT="$sdk"

"$builder" compile "$root_dir/compiler_seen/src/main_compiler.seen" \
    "$compiler_out" --target macos-arm64 --fast --no-cache \
    --jobs 1 --opt-jobs 1 --no-fork
file "$compiler_out" | grep -Eq 'Mach-O 64-bit arm64 executable' ||
    die 'compiler output is not Mach-O arm64'

"$script_dir/build_package_client.sh" --version "$version" \
    --goos darwin --goarch arm64 --output "$package_out"
"$script_dir/package_macos_release.sh" "$version" "$compiler_out" \
    "$package_out" "$output_dir"
printf 'macOS SDK settings SHA-256: '
sha256sum "$sdk/SDKSettings.json" | awk '{print $1}'
