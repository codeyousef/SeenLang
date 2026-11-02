#!/usr/bin/env bash
set -euo pipefail
name=${0##*/}
if [ "$name" = pkg-config ] || [ "$name" = pkgconf ]; then
    arguments=" ${*,,} "
    case "$arguments" in
        *cuda*|*cublas*|*vulkan*|*rocm*|*hip*|*opencl*|*metal*) ;;
        *) exec "${SEEN_CPU_ISOLATION_REAL_PKG_CONFIG:?}" "$@" ;;
    esac
fi
# This is a rejecting test tripwire, never a substitute SDK implementation.
printf '%s' "$name" >> "${SEEN_CPU_ISOLATION_TOOL_LOG:?}"
printf ' <%s>' "$@" >> "$SEEN_CPU_ISOLATION_TOOL_LOG"
printf '\n' >> "$SEEN_CPU_ISOLATION_TOOL_LOG"
exit 97
