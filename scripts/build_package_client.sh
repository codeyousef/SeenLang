#!/usr/bin/env bash
# Build the version-coupled Seen package client with a bounded address space.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PACKAGE_DIR="$ROOT_DIR/tools/seen-pkg"

VERSION=""
TARGET_OS="${GOOS:-}"
TARGET_ARCH="${GOARCH:-}"
OUTPUT=""
GO_BIN="${SEEN_GO:-}"
VMEM_KB="${SEEN_PACKAGE_CLIENT_VMEM_KB:-}"

if [[ -z "$VMEM_KB" ]]; then
    available_kb=""
    if [[ -r /proc/meminfo ]]; then
        available_kb="$(awk '$1 == "MemAvailable:" { print $2; exit }' /proc/meminfo)"
    elif command -v sysctl >/dev/null 2>&1; then
        available_bytes="$(sysctl -n hw.memsize 2>/dev/null || true)"
        if [[ "$available_bytes" =~ ^[0-9]+$ ]]; then
            available_kb="$((available_bytes / 1024))"
        fi
    fi
    if [[ ! "$available_kb" =~ ^[0-9]+$ ]]; then
        echo "Error: cannot derive a safe package-client build cap; set SEEN_PACKAGE_CLIENT_VMEM_KB." >&2
        exit 1
    fi
    VMEM_KB="$((available_kb / 2))"
    if [[ "$VMEM_KB" -gt 2097152 ]]; then VMEM_KB=2097152; fi
    if [[ "$VMEM_KB" -lt 524288 ]]; then VMEM_KB=524288; fi
fi

if [[ -z "$TARGET_OS" ]]; then
    case "$(uname -s)" in
        Linux) TARGET_OS="linux" ;;
        Darwin) TARGET_OS="darwin" ;;
        MINGW*|MSYS*|CYGWIN*) TARGET_OS="windows" ;;
    esac
fi
if [[ -z "$TARGET_ARCH" ]]; then
    case "$(uname -m)" in
        x86_64|amd64) TARGET_ARCH="amd64" ;;
        aarch64|arm64) TARGET_ARCH="arm64" ;;
        riscv64) TARGET_ARCH="riscv64" ;;
    esac
fi

usage() {
    echo "Usage: $0 --version <version> [--goos <os>] [--goarch <arch>] [--output <path>]"
    echo ""
    echo "Environment:"
    echo "  SEEN_GO                      Go executable (default: go from PATH)"
    echo "  SEEN_PACKAGE_CLIENT_VMEM_KB  Address-space cap (default: half available memory, max 2097152)"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --goos) TARGET_OS="$2"; shift 2 ;;
        --goarch) TARGET_ARCH="$2"; shift 2 ;;
        --output) OUTPUT="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ -z "$VERSION" || -z "$TARGET_OS" || -z "$TARGET_ARCH" ]]; then
    usage >&2
    exit 2
fi

if [[ -z "$GO_BIN" ]]; then
    GO_BIN="$(command -v go 2>/dev/null || true)"
fi
if [[ -z "$GO_BIN" || ! -x "$GO_BIN" ]]; then
    echo "Error: Go is required to build the Seen package client." >&2
    echo "Set SEEN_GO to a Go 1.26 executable." >&2
    exit 1
fi
if [[ ! "$VMEM_KB" =~ ^[0-9]+$ ]] || [[ "$VMEM_KB" -lt 524288 ]]; then
    echo "Error: SEEN_PACKAGE_CLIENT_VMEM_KB must be an integer of at least 524288 KiB." >&2
    exit 2
fi

if [[ -z "$OUTPUT" ]]; then
    suffix=""
    [[ "$TARGET_OS" == "windows" ]] && suffix=".exe"
    OUTPUT="$PACKAGE_DIR/bin/seen-pkg${suffix}"
fi
if [[ "$OUTPUT" != /* ]]; then
    OUTPUT="$(pwd)/$OUTPUT"
fi
mkdir -p "$(dirname "$OUTPUT")"

module_version="$(awk '$1 == "module" { print $2; exit }' "$PACKAGE_DIR/go.mod")"
if [[ "$module_version" != "github.com/codeyousef/seen/tools/seen-pkg" ]]; then
    echo "Error: unexpected package-client module identity: $module_version" >&2
    exit 1
fi
declared_version="$(awk -F'"' '/SidecarVersion[[:space:]]*=/{print $2; exit}' \
    "$PACKAGE_DIR/internal/commands/request.go")"
if [[ "$declared_version" != "$VERSION" ]]; then
    echo "Error: package-client source is version $declared_version, requested $VERSION." >&2
    exit 1
fi

tmp_output="${OUTPUT}.tmp.$$"
trap 'rm -f "$tmp_output"' EXIT

echo "Building seen-pkg $VERSION for $TARGET_OS/$TARGET_ARCH (VMEM ${VMEM_KB} KiB)..."
(
    cd "$PACKAGE_DIR"
    build_command=(
        env CGO_ENABLED=0 GOOS="$TARGET_OS" GOARCH="$TARGET_ARCH"
        GOMAXPROCS="${GOMAXPROCS:-2}" GOFLAGS="${GOFLAGS:--p=1}"
        "$GO_BIN" build -mod=readonly -trimpath -buildvcs=false
        -ldflags="-s -w" -o "$tmp_output" ./cmd/seen-pkg
    )
    if command -v prlimit >/dev/null 2>&1; then
        exec prlimit --as="$((VMEM_KB * 1024))" -- "${build_command[@]}"
    fi
    if ! ulimit -v "$VMEM_KB" 2>/dev/null; then
        echo "Error: cannot enforce the package-client build cap with prlimit or ulimit." >&2
        exit 1
    fi
    exec "${build_command[@]}"
)

chmod 755 "$tmp_output"
mv -f "$tmp_output" "$OUTPUT"
trap - EXIT
echo "Built $OUTPUT"
