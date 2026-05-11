#!/usr/bin/env bash
# Check or install the external LLVM toolchain used by Seen packages.

set -euo pipefail

LLVM_MIN_VERSION="${SEEN_LLVM_MIN_VERSION:-18}"
LLVM_PREFERRED_VERSION="${SEEN_LLVM_PREFERRED_VERSION:-18}"
MODE="check"
PREFIX="${SEEN_PREFIX:-}"
QUIET=0

usage() {
    echo "Usage: $0 [--check|--install|--print-env] [--prefix DIR] [--quiet]"
    echo ""
    echo "Environment:"
    echo "  SEEN_PREFIX              Seen installation prefix"
    echo "  SEEN_LLVM_BIN            Preferred LLVM bin directory"
    echo "  SEEN_LLVM_MIN_VERSION    Minimum LLVM major version (default: 18)"
    echo "  SEEN_LLVM_PREFERRED_VERSION Preferred managed LLVM major (default: 18)"
    echo "  SEEN_SKIP_TOOLCHAIN=1    Skip all checks"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) MODE="check"; shift ;;
        --install) MODE="install"; shift ;;
        --print-env) MODE="print-env"; shift ;;
        --prefix) PREFIX="$2"; shift 2 ;;
        --quiet) QUIET=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ "${SEEN_SKIP_TOOLCHAIN:-0}" == "1" ]]; then
    [[ "$QUIET" == "1" ]] || echo "Seen LLVM toolchain check skipped."
    exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -z "$PREFIX" ]]; then
    if [[ "$SCRIPT_DIR" == */lib/seen/toolchain ]]; then
        PREFIX="$(cd "$SCRIPT_DIR/../../.." && pwd)"
    else
        PREFIX="$(cd "$SCRIPT_DIR/.." && pwd)"
    fi
fi

log() {
    if [[ "$QUIET" != "1" ]]; then
        echo "$@"
    fi
}

add_search_dir() {
    local dir="$1"
    if [[ -z "$dir" || ! -d "$dir" ]]; then
        return
    fi
    case ":$SEARCH_DIRS:" in
        *":$dir:"*) ;;
        *) SEARCH_DIRS="${SEARCH_DIRS:+$SEARCH_DIRS:}$dir" ;;
    esac
}

SEARCH_DIRS=""
add_search_dir "$PREFIX/lib/seen/toolchain/llvm/bin"
add_search_dir "${SEEN_LLVM_BIN:-}"
add_search_dir "/opt/homebrew/opt/llvm/bin"
add_search_dir "/usr/local/opt/llvm/bin"

IFS=':' read -r -a PATH_DIRS <<< "${PATH:-}"
for dir in "${PATH_DIRS[@]}"; do
    add_search_dir "$dir"
done

candidate_names() {
    local base="$1"
    local versions=("$LLVM_PREFERRED_VERSION" 20 19 18)
    local names=()
    local version=""
    for version in "${versions[@]}"; do
        names+=("$base-$version")
    done
    names+=("$base")
    if [[ "$base" == "ld.lld" ]]; then
        for version in "${versions[@]}"; do
            names+=("lld-$version")
        done
        names+=("lld")
    fi
    printf '%s\n' "${names[@]}"
}

find_tool() {
    local base="$1"
    local name=""
    local dir=""
    while IFS= read -r name; do
        IFS=':' read -r -a DIRS <<< "$SEARCH_DIRS"
        for dir in "${DIRS[@]}"; do
            if [[ -x "$dir/$name" ]]; then
                echo "$dir/$name"
                return 0
            fi
        done
        if command -v "$name" >/dev/null 2>&1; then
            command -v "$name"
            return 0
        fi
    done < <(candidate_names "$base")
    return 1
}

tool_major_version() {
    local tool="$1"
    "$tool" --version 2>/dev/null |
        sed -n '1s/.*version \([0-9][0-9]*\).*/\1/p;1s/.*LLVM \([0-9][0-9]*\).*/\1/p;1s/.*LLD \([0-9][0-9]*\).*/\1/p' |
        head -n 1
}

CHECK_TOOLS=(clang opt llc llvm-as ld.lld)
FOUND_PATHS=()
FOUND_DIRS=""
MISSING=()
OUTDATED=()

for base in "${CHECK_TOOLS[@]}"; do
    if path="$(find_tool "$base")"; then
        major="$(tool_major_version "$path")"
        if [[ -n "$major" && "$major" -lt "$LLVM_MIN_VERSION" ]]; then
            OUTDATED+=("$base ($path reports LLVM $major)")
        else
            FOUND_PATHS+=("$base=$path")
            dir="$(dirname "$path")"
            case ":$FOUND_DIRS:" in
                *":$dir:"*) ;;
                *) FOUND_DIRS="${FOUND_DIRS:+$FOUND_DIRS:}$dir" ;;
            esac
        fi
    else
        MISSING+=("$base")
    fi
done

if [[ "${#MISSING[@]}" -eq 0 && "${#OUTDATED[@]}" -eq 0 ]]; then
    if [[ "$MODE" == "print-env" ]]; then
        echo "export PATH=\"$FOUND_DIRS:\$PATH\""
        if [[ "$FOUND_DIRS" != *:* ]]; then
            echo "export SEEN_LLVM_BIN=\"$FOUND_DIRS\""
        fi
    else
        log "Seen LLVM toolchain ready:"
        for item in "${FOUND_PATHS[@]}"; do
            log "  $item"
        done
    fi
    exit 0
fi

print_problem() {
    if [[ "${#MISSING[@]}" -gt 0 ]]; then
        echo "Missing LLVM tools: ${MISSING[*]}" >&2
    fi
    if [[ "${#OUTDATED[@]}" -gt 0 ]]; then
        echo "LLVM tools below required version $LLVM_MIN_VERSION:" >&2
        for item in "${OUTDATED[@]}"; do
            echo "  $item" >&2
        done
    fi
}

install_with_package_manager() {
    local os_name
    os_name="$(uname -s)"
    if [[ "$os_name" == "Darwin" ]]; then
        if command -v brew >/dev/null 2>&1; then
            brew install llvm
            return 0
        fi
        echo "Homebrew is required for managed LLVM installation on macOS." >&2
        return 1
    fi

    if [[ "$os_name" != "Linux" ]]; then
        echo "Managed LLVM installation is not available for $os_name." >&2
        return 1
    fi

    if command -v apt-get >/dev/null 2>&1; then
        sudo apt-get update
        sudo apt-get install -y "llvm-$LLVM_PREFERRED_VERSION" \
            "clang-$LLVM_PREFERRED_VERSION" "lld-$LLVM_PREFERRED_VERSION"
        return 0
    fi
    if command -v dnf >/dev/null 2>&1; then
        sudo dnf install -y llvm clang lld
        return 0
    fi
    if command -v yum >/dev/null 2>&1; then
        sudo yum install -y llvm clang lld
        return 0
    fi
    if command -v pacman >/dev/null 2>&1; then
        sudo pacman -S --needed --noconfirm llvm clang lld
        return 0
    fi

    echo "No supported package manager found for managed LLVM installation." >&2
    return 1
}

if [[ "$MODE" == "install" ]]; then
    print_problem
    log "Installing managed LLVM toolchain..."
    install_with_package_manager
    exec "$0" --check --prefix "$PREFIX"
fi

print_problem
echo "Install LLVM $LLVM_MIN_VERSION+ and ensure clang, opt, llc, llvm-as, and lld are on PATH." >&2
echo "Or rerun with SEEN_MANAGED_TOOLCHAIN=1 / --install for a managed install attempt." >&2
exit 1
