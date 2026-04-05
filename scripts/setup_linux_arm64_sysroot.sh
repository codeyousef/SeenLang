#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DEST_DIR="$ROOT_DIR/artifacts/toolchains/linux-arm64"
DOWNLOAD_DIR=""
FORCE=0

PACKAGES=(
  aarch64-linux-gnu-binutils
  aarch64-linux-gnu-gcc
  aarch64-linux-gnu-linux-api-headers
  aarch64-linux-gnu-glibc
)

usage() {
  cat <<'USAGE'
setup_linux_arm64_sysroot.sh - create a local Linux ARM64 cross sysroot for Seen.

Usage: scripts/setup_linux_arm64_sysroot.sh [options]

Options:
  --dest <dir>          Destination root for the extracted toolchain/sysroot
  --download-dir <dir>  Directory for downloaded package archives
  --force               Replace an existing destination directory
  -h, --help            Show this help message

This helper currently targets pacman-compatible hosts (Arch/CachyOS/etc.).
It resolves package URLs with pacman, downloads the ARM64 cross packages, and
extracts them into a local directory so Seen can use:

  SEEN_LINUX_ARM64_SYSROOT
  SEEN_LINUX_ARM64_GCC_TOOLCHAIN
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dest)
      DEST_DIR="$2"
      shift 2
      ;;
    --download-dir)
      DOWNLOAD_DIR="$2"
      shift 2
      ;;
    --force)
      FORCE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Required command not found: $cmd" >&2
    exit 1
  fi
}

require_cmd pacman
require_cmd curl
require_cmd bsdtar

if [[ -z "$DOWNLOAD_DIR" ]]; then
  DOWNLOAD_DIR="$ROOT_DIR/artifacts/toolchains/packages/linux-arm64"
fi

DEST_DIR="$(mkdir -p "$(dirname "$DEST_DIR")" && cd "$(dirname "$DEST_DIR")" && pwd)/$(basename "$DEST_DIR")"
DOWNLOAD_DIR="$(mkdir -p "$DOWNLOAD_DIR" && cd "$DOWNLOAD_DIR" && pwd)"

if [[ -e "$DEST_DIR" && "$FORCE" -ne 1 ]]; then
  echo "Destination already exists: $DEST_DIR" >&2
  echo "Use --force to replace it." >&2
  exit 1
fi

if [[ "$DEST_DIR" == "/" || "$DEST_DIR" == "" ]]; then
  echo "Refusing to use destination: $DEST_DIR" >&2
  exit 1
fi

echo "[linux-arm64-sysroot] resolving package URLs via pacman"
mapfile -t package_urls < <(pacman -Sp -- "${PACKAGES[@]}")
if [[ ${#package_urls[@]} -ne ${#PACKAGES[@]} ]]; then
  echo "Failed to resolve package URLs for the expected ARM64 cross packages." >&2
  exit 1
fi

downloaded_archives=()
for url in "${package_urls[@]}"; do
  archive_name="$(basename "$url")"
  archive_path="$DOWNLOAD_DIR/$archive_name"
  if [[ ! -f "$archive_path" ]]; then
    echo "[linux-arm64-sysroot] downloading $archive_name"
    curl -L --fail --output "$archive_path" "$url"
  else
    echo "[linux-arm64-sysroot] reusing $archive_name"
  fi
  downloaded_archives+=("$archive_path")
done

staging_dir="$DEST_DIR.tmp.$$"
rm -rf "$staging_dir"
mkdir -p "$staging_dir"

echo "[linux-arm64-sysroot] extracting packages into $DEST_DIR"
for archive_path in "${downloaded_archives[@]}"; do
  bsdtar -C "$staging_dir" -xf "$archive_path"
done

sysroot_dir="$staging_dir/usr/aarch64-linux-gnu"
gcc_toolchain_dir="$staging_dir/usr"
env_file="$staging_dir/env.sh"
final_sysroot_dir="$DEST_DIR/usr/aarch64-linux-gnu"
final_gcc_toolchain_dir="$DEST_DIR/usr"

if [[ ! -x "$gcc_toolchain_dir/bin/aarch64-linux-gnu-gcc" ]]; then
  echo "Extracted toolchain is missing aarch64-linux-gnu-gcc under $gcc_toolchain_dir/bin" >&2
  rm -rf "$staging_dir"
  exit 1
fi

if [[ ! -d "$sysroot_dir/lib" ]]; then
  echo "Extracted sysroot is missing $sysroot_dir/lib" >&2
  rm -rf "$staging_dir"
  exit 1
fi

cat > "$env_file" <<EOF
export SEEN_LINUX_ARM64_SYSROOT="$final_sysroot_dir"
export SEEN_LINUX_ARM64_GCC_TOOLCHAIN="$final_gcc_toolchain_dir"
EOF

rm -rf "$DEST_DIR"
mv "$staging_dir" "$DEST_DIR"

echo "[linux-arm64-sysroot] ready"
echo "  sysroot: $DEST_DIR/usr/aarch64-linux-gnu"
echo "  gcc toolchain: $DEST_DIR/usr"
echo "  env file: $DEST_DIR/env.sh"
echo
echo "Next steps:"
echo "  source \"$DEST_DIR/env.sh\""
echo "  bash scripts/native_target_smoke.sh --compiler compiler_seen/target/seen --target linux-arm64"
