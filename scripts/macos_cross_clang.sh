#!/usr/bin/env bash
# PATH-injected clang for a serial, Linux-contained macOS arm64 cross-build.
set -euo pipefail

sdk="${SEEN_MACOS_SDKROOT:-}"
[[ "$sdk" == /* && -d "$sdk/usr/include" && -f "$sdk/SDKSettings.json" ]] || {
    echo 'macOS cross-clang: SEEN_MACOS_SDKROOT must name a complete absolute SDK' >&2
    exit 126
}
[[ "$(uname -s)" == Linux ]] || {
    echo 'macOS cross-clang: only the verified Linux cross-build path is supported' >&2
    exit 126
}

for arg in "$@"; do
    case "$arg" in
        -c|-S|-E)
            exec /usr/bin/clang -isysroot "$sdk" -D_DARWIN_C_SOURCE \
                "$@"
            ;;
    esac
done
exec /usr/bin/clang -isysroot "$sdk" -D_DARWIN_C_SOURCE \
    -fuse-ld=lld -Wl,--threads=1 -Wl,--thinlto-jobs=1 "$@"
