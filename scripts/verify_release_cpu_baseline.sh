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
  - native binaries do not require AVX-512 (runtime-dispatched Go paths are allowed)
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

go_package_client_has_compatible_baseline() {
    local bin="$1"
    local goamd64=""
    local level=""

    [[ "$(basename "$bin")" == "seen-pkg" ]] || return 1
    grep -aFq $'path\tgithub.com/codeyousef/seen/tools/seen-pkg/cmd/seen-pkg' "$bin" || return 1
    grep -aFq $'build\tCGO_ENABLED=0' "$bin" || return 1
    grep -aFq $'build\tGOARCH=amd64' "$bin" || return 1
    grep -aFq $'build\tGOOS=linux' "$bin" || return 1

    for level in v1 v2 v3 v4; do
        if grep -aFq $'build\t'"GOAMD64=$level" "$bin"; then
            [[ -z "$goamd64" ]] || return 1
            goamd64="$level"
        fi
    done
    [[ -n "$goamd64" ]] || return 1

    case "$CPU_BASELINE:$goamd64" in
        x86-64:v1|x86-64-v3:v1|x86-64-v3:v2|x86-64-v3:v3) return 0 ;;
        *) return 1 ;;
    esac
}

go_package_client_avx512_is_runtime_dispatched() {
    local disassembly="$1"

    awk '
        function allowed(name) {
            if (name ~ /^expandAVX512_[0-9]+$/) return 1
            if (name ~ /^internal\/runtime\/gc\/scan\.(FilterNilAVX512\.abi0|scanSpanPackedAVX512\.abi0)$/) return 1
            if (name == "runtime.asyncPreempt.abi0") return 1
            if (name == "hash/crc32.ieeeCLMUL.abi0") return 1
            return 0
        }
        /^[[:space:]]*[[:xdigit:]]+ <[^>]+>:/ {
            current = $0
            sub(/^[^<]*</, "", current)
            sub(/>:.*/, "", current)
        }
        {
            lower = tolower($0)
            evex = lower ~ /^[[:space:]]*[[:xdigit:]]+:[[:space:]]+62[[:space:]]/
            registers = lower ~ /(%zmm[0-9]+|%k[0-7]([^[:alnum:]_]|$))/
            if ((evex || registers) && !allowed(current)) {
                print "Unapproved AVX-512 evidence in " current ": " $0 > "/dev/stderr"
                failed = 1
            }
        }
        END { exit failed ? 1 : 0 }
    ' "$disassembly"
}

scan_for_avx512() {
    local bin="$1"
    local disassembly=""

    if ! command -v file >/dev/null 2>&1 || ! command -v objdump >/dev/null 2>&1; then
        echo "Warning: instruction scan skipped because file or objdump is unavailable" >&2
        return 0
    fi

    if ! file "$bin" | grep -Eq 'ELF|Mach-O|PE32'; then
        echo "Instruction scan skipped for non-native test binary: $bin"
        return 0
    fi

    disassembly="$(mktemp /tmp/seen_release_objdump.XXXXXX)"
    # x86 instructions are at most 15 bytes, so width 16 prevents wrapped raw-byte
    # continuations from looking like EVEX (0x62) instruction starts.
    if ! objdump -d --insn-width=16 "$bin" > "$disassembly"; then
        echo "Unable to disassemble native release binary: $bin" >&2
        rm -f "$disassembly"
        return 1
    fi

    if grep -Eiq '(^[[:space:]]*[[:xdigit:]]+:[[:space:]]+62[[:space:]]|%zmm[0-9]+|%k[0-7]([^[:alnum:]_]|$))' \
        "$disassembly"; then
        if go_package_client_has_compatible_baseline "$bin" && \
            go_package_client_avx512_is_runtime_dispatched "$disassembly"; then
            echo "Accepted runtime-dispatched AVX-512 in baseline-compatible Go package client: $bin"
            rm -f "$disassembly"
            return 0
        fi
        echo "AVX-512-only instruction evidence found in $bin" >&2
        rm -f "$disassembly"
        return 1
    fi
    rm -f "$disassembly"
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

    if ! go_package_client_has_compatible_baseline "$package_client"; then
        echo "Package-client Go CPU baseline is incompatible with $CPU_BASELINE: $package_client" >&2
        exit 1
    fi

    if ! "$package_client" --expect-version "$release_version" version >/dev/null 2>&1; then
        echo "Package-client version handshake failed for Seen $release_version" >&2
        exit 1
    fi

    scan_for_avx512 "$bin"
    scan_for_avx512 "$package_client"

    run_smoke_tests "$bin" "$tmpdir"
    rm -rf "$tmpdir"
    trap - EXIT

    echo "OK: $(basename "$artifact") satisfies $CPU_BASELINE release checks"
done
