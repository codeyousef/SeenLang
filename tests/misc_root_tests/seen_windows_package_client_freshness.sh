#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_windows_pkg_freshness.XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

make_fixture() {
    local name="$1"
    local source_script="$2"
    local fixture="$TMP_DIR/$name"
    mkdir -p "$fixture/scripts" "$fixture/target-windows"
    cp "$ROOT_DIR/scripts/$source_script" "$fixture/scripts/$source_script"
    printf 'stale helper\n' > "$fixture/target-windows/seen-pkg.exe"
    cat > "$fixture/scripts/build_package_client.sh" <<'BUILD_EOF'
#!/usr/bin/env bash
set -euo pipefail
version=""
goos=""
goarch=""
output=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) version="$2"; shift 2 ;;
        --goos) goos="$2"; shift 2 ;;
        --goarch) goarch="$2"; shift 2 ;;
        --output) output="$2"; shift 2 ;;
        *) exit 2 ;;
    esac
done
[[ "$version" == "0.10.0" ]]
[[ "$goos" == "windows" ]]
[[ "$goarch" == "amd64" ]]
printf 'fresh 0.10.0 helper\n' > "$output"
BUILD_EOF
    chmod 755 "$fixture/scripts/build_package_client.sh"
    printf '%s\n' "$fixture"
}

installer_fixture="$(make_fixture installer build_windows_installer.sh)"
if bash "$installer_fixture/scripts/build_windows_installer.sh" \
    0.10.0 --skip-compile >/dev/null 2>&1; then
    echo "Windows installer fixture unexpectedly found a compiler" >&2
    exit 1
fi
grep -qx 'fresh 0.10.0 helper' \
    "$installer_fixture/target-windows/seen-pkg.exe"

package_fixture="$(make_fixture package package_windows.sh)"
if bash "$package_fixture/scripts/package_windows.sh" \
    0.10.0 >/dev/null 2>&1; then
    echo "Windows package fixture unexpectedly found a compiler" >&2
    exit 1
fi
grep -qx 'fresh 0.10.0 helper' \
    "$package_fixture/target-windows/seen-pkg.exe"

echo "Windows package-client freshness test passed"
