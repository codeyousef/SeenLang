#!/usr/bin/env bash
# Verify packaged Linux release binaries against the declared CPU baseline.

set -euo pipefail

CPU_BASELINE="${SEEN_RELEASE_CPU_BASELINE:-x86-64}"
ARTIFACTS=()

usage() {
    cat <<'USAGE'
Usage: verify_release_cpu_baseline.sh [--cpu-baseline <x86-64|x86-64-v3>] <artifact.tar.gz>...

Checks:
  - package metadata declares the requested CPU baseline
  - version-coupled package client is present and accepts the release version
  - default x86-64 packages do not contain AVX-512-only instruction evidence
  - packaged compiler starts, runs `check`, and compiles a small program
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --cpu-baseline)
            CPU_BASELINE="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -*)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
        *)
            ARTIFACTS+=("$1")
            shift
            ;;
    esac
done

case "$CPU_BASELINE" in
    x86-64|x86-64-v3) ;;
    *)
        echo "Unsupported CPU baseline: $CPU_BASELINE" >&2
        exit 2
        ;;
esac

if [[ ${#ARTIFACTS[@]} -eq 0 ]]; then
    echo "No artifacts supplied" >&2
    usage >&2
    exit 2
fi

scan_for_avx512() {
    local bin="$1"

    if ! command -v file >/dev/null 2>&1 || ! command -v objdump >/dev/null 2>&1; then
        echo "Warning: instruction scan skipped because file or objdump is unavailable" >&2
        return 0
    fi

    if ! file "$bin" | grep -Eq 'ELF|Mach-O|PE32'; then
        echo "Instruction scan skipped for non-native test binary: $bin"
        return 0
    fi

    if objdump -d "$bin" | grep -Eiq '\b(zmm[0-9]+|%zmm[0-9]+|k[0-7]\{|%k[0-7]\{|avx512|vgather.*zmm|vscatter.*zmm)\b'; then
        echo "AVX-512-only instruction evidence found in $bin" >&2
        return 1
    fi
}

run_smoke_tests() {
    local bin="$1"
    local tmpdir="$2"
    local source="$tmpdir/release_smoke.seen"
    local output="$tmpdir/release_smoke_bin"
    local pkgdir="$tmpdir/prebuild_pkg"
    local artifact_dir="$tmpdir/prebuild_artifact"
    local pkg_output
    local startup_output

    if "$bin" --version >/dev/null 2>&1; then
        :
    elif "$bin" >/dev/null 2>&1; then
        :
    else
        startup_output="$("$bin" 2>&1 || true)"
        if ! grep -qi 'usage' <<<"$startup_output"; then
            echo "Packaged compiler did not start cleanly: $bin" >&2
            return 1
        fi
    fi

    cat > "$source" <<'SMOKE_EOF'
fun main() r: Int {
    println("release smoke")
    return 0
}
SMOKE_EOF

    "$bin" check "$source" >/dev/null 2>&1
    "$bin" compile "$source" "$output" --fast --no-cache --target-cpu="$CPU_BASELINE" >/dev/null 2>&1

    pkg_output="$("$bin" pkg 2>&1 || true)"
    if ! grep -q 'pkg prebuild' <<<"$pkg_output"; then
        echo "Packaged compiler does not expose 'seen pkg prebuild': $bin" >&2
        return 1
    fi

    mkdir -p "$pkgdir/src" "$artifact_dir"
    cat > "$pkgdir/Seen.toml" <<'PKG_TOML_EOF'
[project]
name = "release_prebuild_smoke"
version = "0.1.0"
language = "en"
modules = ["src/value.seen"]
PKG_TOML_EOF
    cat > "$pkgdir/src/value.seen" <<'PKG_SRC_EOF'
pub fun release_prebuild_value() r: Int {
    return 7
}
PKG_SRC_EOF

    "$bin" pkg prebuild "$pkgdir" "$artifact_dir" >/dev/null 2>&1
    if [[ ! -s "$artifact_dir/objects.tsv" ]]; then
        echo "Package prebuild smoke did not emit objects.tsv: $bin" >&2
        return 1
    fi
    if [[ ! -s "$artifact_dir/interface.index.tsv" ]]; then
        echo "Package prebuild smoke did not emit interface.index.tsv: $bin" >&2
        return 1
    fi
}

for artifact in "${ARTIFACTS[@]}"; do
    if [[ ! -f "$artifact" ]]; then
        echo "Artifact not found: $artifact" >&2
        exit 1
    fi

    case "$artifact" in
        *.tar.gz) ;;
        *)
            echo "Unsupported artifact type for CPU baseline verification: $artifact" >&2
            exit 1
            ;;
    esac

    tmpdir="$(mktemp -d /tmp/seen_release_verify.XXXXXX)"
    trap 'rm -rf "$tmpdir"' EXIT

    tar -xzf "$artifact" -C "$tmpdir"
    package_dir="$(find "$tmpdir" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
    if [[ -z "$package_dir" ]]; then
        echo "Artifact did not contain a package directory: $artifact" >&2
        exit 1
    fi

    bin="$package_dir/bin/seen"
    package_client="$package_dir/bin/seen-pkg"
    metadata="$package_dir/share/doc/seen/release-cpu-baseline.txt"

    if [[ ! -x "$bin" ]]; then
        echo "Packaged compiler missing or not executable: $bin" >&2
        exit 1
    fi
    if [[ ! -x "$package_client" ]]; then
        echo "Packaged package client missing or not executable: $package_client" >&2
        exit 1
    fi
    if [[ ! -f "$metadata" ]]; then
        echo "Missing release CPU metadata: $metadata" >&2
        exit 1
    fi
    if ! grep -qx "cpu-baseline=$CPU_BASELINE" "$metadata"; then
        echo "CPU baseline metadata mismatch in $metadata" >&2
        cat "$metadata" >&2
        exit 1
    fi
    release_version=""
    while IFS='=' read -r metadata_key metadata_value; do
        if [[ "$metadata_key" == "version" ]]; then
            release_version="$metadata_value"
            break
        fi
    done < "$metadata"
    if [[ -z "$release_version" ]]; then
        echo "Release metadata does not declare a version: $metadata" >&2
        exit 1
    fi

    compiler_version_output=""
    if ! compiler_version_output="$("$bin" --version 2>&1)"; then
        echo "Packaged compiler could not report its version: $bin" >&2
        exit 1
    fi
    compiler_version_line="${compiler_version_output%%$'\n'*}"
    compiler_version_line="${compiler_version_line%$'\r'}"
    if [[ "$compiler_version_line" != "Seen $release_version" ]]; then
        echo "Compiler version mismatch: release metadata expects 'Seen $release_version', got '$compiler_version_line'" >&2
        exit 1
    fi

    if ! "$package_client" --expect-version "$release_version" version >/dev/null 2>&1; then
        echo "Package-client version handshake failed for Seen $release_version" >&2
        exit 1
    fi

    if [[ "$CPU_BASELINE" == "x86-64" ]]; then
        scan_for_avx512 "$bin"
        scan_for_avx512 "$package_client"
    fi

    run_smoke_tests "$bin" "$tmpdir"
    rm -rf "$tmpdir"
    trap - EXIT

    echo "OK: $(basename "$artifact") satisfies $CPU_BASELINE release checks"
done
