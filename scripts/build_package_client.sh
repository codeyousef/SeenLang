#!/usr/bin/env bash
# Build the version-coupled Seen package client with a bounded address space.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PACKAGE_DIR="$ROOT_DIR/tools/seen-pkg"
ARTIFACT_ROOT_SCRIPT="$SCRIPT_DIR/artifact_root.sh"
ARTIFACT_WRAPPER="$SCRIPT_DIR/run_with_project_artifacts.sh"
HARD_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
ORIGINAL_ARGS=("$@")

VERSION=""
TARGET_OS="${GOOS:-}"
TARGET_ARCH="${GOARCH:-}"
OUTPUT=""
GO_BIN="${SEEN_GO:-}"
VMEM_KB="${SEEN_PACKAGE_CLIENT_VMEM_KB:-}"

if [[ ! -f "$ARTIFACT_ROOT_SCRIPT" ]]; then
    echo "Error: missing artifact-root helper: $ARTIFACT_ROOT_SCRIPT" >&2
    exit 1
fi
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_SCRIPT"
seen_artifact_root_init "$ROOT_DIR"

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
if [[ ! "$VMEM_KB" =~ ^[0-9]+$ ]] || [[ "$VMEM_KB" -lt 524288 ]] ||
    [[ "$VMEM_KB" -gt 2097152 ]]; then

    echo "Error: SEEN_PACKAGE_CLIENT_VMEM_KB must be between 524288 and 2097152 KiB." >&2
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
case "$OUTPUT" in
    "$ROOT_DIR"/*) ;;
    *)
        echo "Error: package-client output must stay inside the repository." >&2
        exit 2
        ;;
esac
output_dir=$(dirname -- "$OUTPUT")
output_dir_relative=${output_dir#"$ROOT_DIR"/}
seen_artifact_assert_safe_relative_path "$output_dir_relative"
seen_artifact_assert_no_symlink_components "$ROOT_DIR" "$output_dir_relative"
mkdir -p -- "$output_dir"
seen_artifact_assert_no_symlink_components "$ROOT_DIR" "$output_dir_relative"
output_relative=${OUTPUT#"$ROOT_DIR"/}
if [ -L "$OUTPUT" ]; then
    echo "Error: package-client output may not be a symbolic link: $OUTPUT" >&2
    exit 2
fi
if command -v git >/dev/null 2>&1 &&
    ! git -C "$ROOT_DIR" check-ignore -q -- "$output_relative"; then

    echo "Error: package-client output must be Git-ignored: $OUTPUT" >&2
    exit 2
fi

if [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" != "1" ] ||
    [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ]; then

    [ -x "$ARTIFACT_WRAPPER" ] || {
        echo "Error: missing project artifact wrapper: $ARTIFACT_WRAPPER" >&2
        exit 1
    }
    exec "$ARTIFACT_WRAPPER" package-client-build -- \
        "$0" "${ORIGINAL_ARGS[@]}"
fi
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then

    [ -x "$HARD_SCOPE_WRAPPER" ] || {
        echo "Error: missing hard-memory-scope wrapper: $HARD_SCOPE_WRAPPER" >&2
        exit 1
    }
    exec "$HARD_SCOPE_WRAPPER" --label "Seen package-client build" -- \
        "$0" "${ORIGINAL_ARGS[@]}"
fi
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
export SEEN_HARD_MEMORY_SCOPE_ACTIVE
"$HARD_SCOPE_WRAPPER" --label "Seen package-client build read-back" \
    --verify-only --

# Initialize Go state only after both the project-local namespace and the hard
# aggregate cgroup have been read back. Validate every cache directory, not
# only their common parent, so a pre-existing symlink cannot redirect writes.
PACKAGE_CACHE_ROOT="$SEEN_ARTIFACT_ROOT/package-client-go"
for package_cache_dir in \
    "$PACKAGE_CACHE_ROOT" \
    "$PACKAGE_CACHE_ROOT/build" \
    "$PACKAGE_CACHE_ROOT/modules" \
    "$PACKAGE_CACHE_ROOT/gopath" \
    "$PACKAGE_CACHE_ROOT/tmp"; do

    case "$package_cache_dir" in
        "$ROOT_DIR"/*) package_cache_relative=${package_cache_dir#"$ROOT_DIR"/} ;;
        *)
            echo "Error: package-client cache escaped the repository." >&2
            exit 1
            ;;
    esac
    seen_artifact_assert_safe_relative_path "$package_cache_relative"
    seen_artifact_assert_no_symlink_components "$ROOT_DIR" "$package_cache_relative"
    [ ! -L "$package_cache_dir" ] || {
        echo "Error: package-client cache is a symbolic link: $package_cache_dir" >&2
        exit 1
    }
    mkdir -p -- "$package_cache_dir"
    seen_artifact_assert_no_symlink_components "$ROOT_DIR" "$package_cache_relative"
done
GOCACHE="$PACKAGE_CACHE_ROOT/build"
GOMODCACHE="$PACKAGE_CACHE_ROOT/modules"
GOPATH="$PACKAGE_CACHE_ROOT/gopath"
GOTMPDIR="$PACKAGE_CACHE_ROOT/tmp"
export GOCACHE GOMODCACHE GOPATH GOTMPDIR

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

tmp_output=$(mktemp "$output_dir/.seen-pkg.XXXXXX")
trap 'rm -f "$tmp_output"' EXIT

echo "Building seen-pkg $VERSION for $TARGET_OS/$TARGET_ARCH (VMEM ${VMEM_KB} KiB)..."
(
    cd "$PACKAGE_DIR"
    target_tuning=()
    if [[ "$TARGET_ARCH" == "amd64" ]]; then
        target_tuning+=(GOAMD64=v1)
    fi
    build_command=(
        env CGO_ENABLED=0 GOOS="$TARGET_OS" GOARCH="$TARGET_ARCH"
        "${target_tuning[@]}"
        GOCACHE="$GOCACHE" GOMODCACHE="$GOMODCACHE" GOPATH="$GOPATH"
        GOTMPDIR="$GOTMPDIR" GOENV=off GOTELEMETRY=off GOTOOLCHAIN=local
        GOMAXPROCS=1 GOFLAGS="-p=1 -modcacherw"
        "$GO_BIN" build -mod=readonly -trimpath -buildvcs=false
        # Keep Go symbols so the release verifier can attribute dispatched ISA paths.
        -ldflags="-w" -o "$tmp_output" ./cmd/seen-pkg
    )
    if command -v prlimit >/dev/null 2>&1; then
        exec prlimit --as="$((VMEM_KB * 1024))" -- "${build_command[@]}"
    fi
    if ! ulimit -S -v "$VMEM_KB" 2>/dev/null; then
        echo "RESOURCE STOP: cannot enforce the package-client build cap with prlimit or ulimit." >&2
        exit 126
    fi
    active_vmem_kb=$(ulimit -S -v 2>/dev/null || true)
    case "$active_vmem_kb" in
        ''|*[!0-9]*)
            echo "RESOURCE STOP: cannot read back the package-client build cap." >&2
            exit 126
            ;;
    esac
    if [ "$active_vmem_kb" -gt "$VMEM_KB" ]; then
        echo "RESOURCE STOP: package-client build cap read-back exceeds ${VMEM_KB} KiB." >&2
        exit 126
    fi
    exec "${build_command[@]}"
)

chmod 755 "$tmp_output"
mv -f "$tmp_output" "$OUTPUT"
trap - EXIT
echo "Built $OUTPUT"
